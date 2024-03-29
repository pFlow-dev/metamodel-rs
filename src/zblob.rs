use serde::Serialize;

use crate::compression::{compress_brotli_encode, decompress_brotli_decode};
use crate::oid::Oid;
use crate::petri_net::PetriNet;

/// `Zblob` is a struct used to pack and unpack a zipped base64 encoded PetriNet into a sharable blob.
#[derive(Debug, Clone, Serialize)]
pub struct Zblob {
    /// The id of the zblob.
    pub id: i64,
    /// The IPFS CID of the zblob.
    pub ipfs_cid: String,
    /// The base64 zipped content of the zblob.
    pub base64_zipped: String,
    /// The title of the zblob.
    pub title: String,
    /// The description of the zblob.
    pub description: String,
    /// The keywords associated with the zblob.
    pub keywords: String,
    /// The referrer of the zblob.
    pub referrer: String,
    /// The creation time of the zblob.
    pub created_at: String,
}

const EMPTY_NET: &str = "UEsDBAoAAAAAAER3WVjjbbhPbAAAAGwAAAAKAAAAbW9kZWwuanNvbnsKICAibW9kZWxUeXBlIjogInBldHJpTmV0IiwKICAidmVyc2lvbiI6ICJ2MCIsCiAgInBsYWNlcyI6IHsKICB9LAogICJ0cmFuc2l0aW9ucyI6IHsKICB9LAogICJhcmNzIjogWwogIF0KfVBLAQIUAAoAAAAAAER3WVjjbbhPbAAAAGwAAAAKAAAAAAAAAAAAAAAAAAAAAABtb2RlbC5qc29uUEsFBgAAAAABAAEAOAAAAJQAAAAAAA==";

impl Default for Zblob {
    fn default() -> Self {
        Self {
            id: 0,
            ipfs_cid: Oid::new(EMPTY_NET.as_bytes()).unwrap().to_string(),
            base64_zipped: EMPTY_NET.to_string(),
            title: "default".to_string(),
            description: "".to_string(),
            keywords: "new".to_string(),
            referrer: "".to_string(),
            created_at: "".to_string(),
        }
    }
}

impl Zblob {
    pub fn from_string(encoded_zip: Option<&str>) -> Self {
        let mut zblob = Zblob::default();
        if encoded_zip.is_some() {
            zblob.base64_zipped = encoded_zip.unwrap().to_string();
            zblob.ipfs_cid = Oid::new(encoded_zip.unwrap().as_bytes())
                .unwrap()
                .to_string();
            zblob.keywords = "".to_string();
        }
        zblob
    }
    pub fn from_net(net: &PetriNet) -> Self {
        let net_json = net.to_json().unwrap();
        let data = compress_brotli_encode(&net_json);
        return Self::from_string(Some(&data));
    }

    pub fn to_net(&self) -> PetriNet {
        let decoded = decompress_brotli_decode(&self.base64_zipped).unwrap();
        return serde_json::from_str(&decoded).unwrap();
    }
}
