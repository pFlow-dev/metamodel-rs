use std::io::Cursor;
use std::io::Read;
use std::io::Write;

use base64::{Engine as _, engine::general_purpose};
use zip::CompressionMethod;
use zip::read::ZipArchive;
use zip::write::FileOptions;

pub fn unzip_encoded(encoded_data: &str, filename: &str) -> Option<String> {
    let decoded = general_purpose::STANDARD.decode(encoded_data);
    if !decoded.is_ok() {
        return None;
    }
    let reader = Cursor::new(decoded.unwrap());
    let mut zip = ZipArchive::new(reader).unwrap();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        if file.name() == filename {
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            return Some(contents);
        }
    }

    None
}

pub fn unzip_encoded_url(url: &str, filename: &str) -> Option<String> {
    let query_string = url.split('?').collect::<Vec<&str>>()[1];
    let z = query_string.split('&').find(|&param| param.starts_with("z="))?;
    let z = &z[2..];

    unzip_encoded(z, filename)
}

pub fn encode_zip(file_data: &str, filename: &str) -> String {
    let writer = Cursor::new(vec![]);
    let mut zip = zip::ZipWriter::new(writer);

    let options = FileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o755);

    zip.start_file(filename, options).unwrap();
    zip.write_all(file_data.to_string().as_bytes()).unwrap();
    let writer = zip.finish().unwrap();

    return general_purpose::STANDARD.encode(writer.into_inner());
}

#[cfg(test)]
mod tests {
    use crate::fixtures::{DINING_PHILOSOPHERS, INHIBIT_TEST};
    use crate::petri_net::read_petri_net_from_string;

    use super::*;

    #[test]
    fn test_unzip_base64_encoded() {
        let encoded = encode_zip(DINING_PHILOSOPHERS, "dining_philosophers.json");
        let decoded = unzip_encoded(&encoded, "dining_philosophers.json").unwrap();
        assert_eq!(decoded, DINING_PHILOSOPHERS);
    }

    #[test]
    fn test_unzip_test_model() {
        let decoded = unzip_encoded_url(&format!("https://example.com/p/?z={}", INHIBIT_TEST), "model.json").unwrap();

        match read_petri_net_from_string(decoded) {
            Ok(net) => {
                assert_eq!(net.arcs.len(), 4);
                assert_eq!(net.places.len(), 1);
                assert_eq!(net.transitions.len(), 4);
            }
            Err(e) => {
                panic!("Failed to read petri net: {}", e);
            }
        }
    }
}