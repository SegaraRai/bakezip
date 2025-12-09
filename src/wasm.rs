use std::io;

use js_sys::{JsString, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Blob;

use crate::zip::{
    inspect::{InspectConfig, InspectedArchive},
    parse::{ZipFile, ZipParseError, ZipReader},
};

#[wasm_bindgen]
pub struct ZipProcessor {
    blob: Blob,
    zip_file: ZipFile,
    warnings: Vec<(Option<u64>, ZipParseError)>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, tsify::Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ZipWarning {
    index: Option<u64>,
    message: String,
}

#[wasm_bindgen]
impl ZipProcessor {
    pub async fn parse(blob: Blob) -> Result<Self, JsValue> {
        let mut reader = JsBlobReader::new(blob.clone());
        let (zip_file, warnings) = ZipFile::parse_with_warnings(&mut reader)
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to parse zip: {e}")))?;
        Ok(ZipProcessor {
            blob,
            zip_file,
            warnings,
        })
    }

    pub fn get_warnings(&self) -> Result<Vec<ZipWarning>, JsValue> {
        let warnings = self
            .warnings
            .iter()
            .map(|(offset, err)| {
                let msg = format!("{err}");
                ZipWarning {
                    index: *offset,
                    message: msg,
                }
            })
            .collect::<Vec<_>>();
        Ok(warnings)
    }

    pub fn inspect(&self, config: &InspectConfig) -> Result<InspectedArchive, JsValue> {
        Ok(InspectedArchive::inspect(&self.zip_file, config))
    }
}

/// WASM reader that streams data from a JavaScript File without buffering the entire file
#[derive(Debug)]
pub struct JsBlobReader(Blob);

impl JsBlobReader {
    pub fn new(blob: Blob) -> Self {
        Self(blob)
    }
}

impl ZipReader for JsBlobReader {
    fn get_size(&mut self) -> impl std::future::Future<Output = Result<u64, io::Error>> {
        let size = self.0.size() as u64;
        async move { Ok(size) }
    }

    fn read(
        &mut self,
        offset: u64,
        size: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, io::Error>> {
        async fn read_blob(blob: &Blob, offset: u64, size: u64) -> Result<Vec<u8>, io::Error> {
            let blob_slice = blob
                .slice_with_f64_and_f64(offset as f64, (offset + size) as f64)
                .map_err(|e| {
                    io::Error::other(format!(
                        "Failed to slice blob {size} bytes from {offset}: {}",
                        JsString::from(e)
                    ))
                })?;

            let array_buffer_promise = blob_slice.array_buffer();
            let array_buffer = JsFuture::from(array_buffer_promise).await.map_err(|e| {
                io::Error::other(format!(
                    "Failed to read blob {size} bytes from {offset}: {}",
                    JsString::from(e)
                ))
            })?;

            let typed_array = Uint8Array::new(&array_buffer);
            Ok(typed_array.to_vec())
        }

        read_blob(&self.0, offset, size)
    }
}
