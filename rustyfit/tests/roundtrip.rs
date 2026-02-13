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
        let mut buf = Vec::<u8>::with_capacity(10_000 >> 10); // 10 MB, large enough to avoid realloc.
        let mut dec = Decoder::new(br);

        'decode: while let Some(fit) = &mut match dec.decode() {
            Ok(fit) => fit,
            Err(err) => {
                if let DecoderError::ChecksumMismatch { .. } = err {
                    if let Some(file_name) = path.file_name() {
                        // NOTE: Doubts exist regarding the integrity of these files.
                        if ["WeightScaleMultiUser.fit", "Settings.fit"]
                            .iter()
                            .any(|x| *x == file_name)
                        {
                            continue 'decode;
                        }
                    }
                }
                panic!("decode: {:?}, err: {:?}", path, err);
            }
        } {
            let mut cursor = Cursor::new(&mut buf);

            let mut enc = EncoderBuilder::new(&mut cursor)
                .endianness(Endianness::BigEndian)
                .build();

            if let Err(err) = enc.encode(fit) {
                panic!("encode: {:?}, err: {:?}", path, err);
            }

            cursor.seek(SeekFrom::Start(0)).unwrap();

            let mut dec = Decoder::new(&mut cursor);
            if let Err(err) = dec.decode() {
                panic!("re-decode the encoded file: {:?}, err: {:?}", path, err);
            }

            buf.clear();
        }
    };

    if let Err(err) = walk_path(&path, &walk_fn) {
        panic!("walk_path: {}", err);
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
