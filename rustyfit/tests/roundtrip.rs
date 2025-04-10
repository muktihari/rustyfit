use std::{fs, io, path::PathBuf};

#[cfg(test)]
#[test]
fn decode_encode_roundtrip() {
    use std::{
        fs::File,
        io::{BufReader, Cursor, Seek, SeekFrom},
        path::Path,
    };

    use rustyfit::{Decoder, DecoderError, EncoderBuilder, Endianness};

    let path = Path::new("tests/data").to_path_buf();

    let walk_fn = |path: &PathBuf| {
        let file = File::open(path).unwrap();
        let br = BufReader::new(file);

        let mut dec = Decoder::new(br);

        let mut fit = match dec.decode() {
            Ok(fit) => fit,
            Err(err) => {
                if let DecoderError::ChecksumMismatch(_, _) = err {
                    // NOTE: Doubts exist regarding the integrity of these files.
                    let exceptions = ["WeightScaleMultiUser.fit", "Settings.fit"];
                    for v in exceptions {
                        if path.file_name().unwrap() == v {
                            return;
                        }
                    }
                }
                panic!("decode: {:?}, err: {:?}", path, err);
            }
        };

        let mut cursor = Cursor::new(Vec::new());

        let mut enc = EncoderBuilder::new(&mut cursor)
            .omit_invalid_value(false)
            .endianness(Endianness::BigEndian)
            .build();

        if let Err(err) = enc.encode(&mut fit) {
            panic!("encode: {:?}, err: {:?}", path, err);
        }

        cursor.seek(SeekFrom::Start(0)).unwrap();

        let mut dec = Decoder::new(&mut cursor);
        if let Err(err) = dec.decode() {
            panic!("re-decode the encoded file: {:?}, err: {:?}", path, err);
        }
    };

    match walk_path(&path, &walk_fn) {
        Ok(_) => {}
        Err(err) => panic!("walk_path: {}", err),
    }
}

fn walk_path<F: Fn(&PathBuf)>(path: &PathBuf, f: &F) -> Result<(), io::Error> {
    let dir = fs::read_dir(path)?;

    for entry in dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_path(&path, f)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "fit") {
            f(&path);
        }
    }
    Ok(())
}
