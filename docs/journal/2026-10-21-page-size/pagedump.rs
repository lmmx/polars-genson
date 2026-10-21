//! Built as its own crate, in release, from `parquet = { version = "60",
//! default-features = false, features = ["zstd", "snap", "lz4"] }`.
//!
//! Every page of a Parquet file, one JSON line each: row group, column, page type (with
//! data page v1/v2), encoding, values, uncompressed bytes (the page reader's buffer) and
//! compressed bytes (the offset index's page size, header included; dictionary pages from
//! the gap before the first data page). Then one line of file metadata.
use parquet::column::page::Page;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::serialized_reader::ReadOptionsBuilder;
use std::fs::File;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let options = ReadOptionsBuilder::new().with_page_index().build();
    let reader = SerializedFileReader::new_with_options(File::open(&path).unwrap(), options).unwrap();
    let md = reader.metadata();
    for rg in 0..md.num_row_groups() {
        let rgm = md.row_group(rg);
        for col in 0..rgm.num_columns() {
            let cm = rgm.column(col);
            let locations = md.page_index_for_row_group(rg).page_locations(col).cloned();
            let mut pages = reader.get_row_group(rg).unwrap().get_column_page_reader(col).unwrap();
            let mut data_index = 0;
            while let Some(page) = pages.get_next_page().unwrap() {
                let (kind, values, encoding, extra) = match &page {
                    Page::DataPage { num_values, encoding, def_level_encoding, .. } => {
                        ("data_v1", *num_values, *encoding, format!("\"def_level_encoding\": \"{def_level_encoding:?}\""))
                    }
                    Page::DataPageV2 { num_values, encoding, def_levels_byte_len, rep_levels_byte_len, is_compressed, .. } => (
                        "data_v2",
                        *num_values,
                        *encoding,
                        format!("\"def_bytes\": {def_levels_byte_len}, \"rep_bytes\": {rep_levels_byte_len}, \"is_compressed\": {is_compressed}"),
                    ),
                    Page::DictionaryPage { num_values, encoding, .. } => ("dict", *num_values, *encoding, String::new()),
                };
                let compressed = if kind == "dict" {
                    locations.as_ref().map(|l| l[0].offset - cm.dictionary_page_offset().unwrap_or(l[0].offset)).unwrap_or(-1)
                } else {
                    let c = locations.as_ref().map(|l| l[data_index].compressed_page_size as i64).unwrap_or(-1);
                    data_index += 1;
                    c
                };
                let sep = if extra.is_empty() { "" } else { ", " };
                println!(
                    "{{\"rg\": {rg}, \"col\": \"{}\", \"kind\": \"{kind}\", \"encoding\": \"{encoding:?}\", \"values\": {values}, \"uncompressed\": {}, \"compressed\": {compressed}{sep}{extra}}}",
                    cm.column_path(),
                    page.buffer().len()
                );
            }
        }
    }
    let fm = md.file_metadata();
    let c0 = md.row_group(0).column(0);
    println!(
        "{{\"file\": true, \"created_by\": {:?}, \"version\": {}, \"row_groups\": {}, \"compression\": \"{:?}\", \"has_column_index\": {}, \"stats\": {}}}",
        fm.created_by().unwrap_or(""),
        fm.version(),
        md.num_row_groups(),
        c0.compression(),
        md.page_index_for_row_group(0).column_index(0).is_some(),
        c0.statistics().is_some()
    );
}
