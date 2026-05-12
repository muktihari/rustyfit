use rustyfit::{Decoder, DecoderError, EncoderBuilder, Endianness, HeaderOption};
use std::error::Error;
use std::{fs, path::PathBuf};
use std::{
    fs::File,
    io::{BufReader, Cursor, Seek, SeekFrom},
    path::Path,
};

#[test]
fn decode_encode_roundtrip() {
    walk_path(
        &Path::new("tests/data").to_path_buf(),
        &mut |path: &PathBuf| {
            do_roudtrip_with_encoder_options(
                path,
                EncoderOptions {
                    endianness: Endianness::LittleEndian,
                    header_option: HeaderOption::Normal(0),
                },
            )
        },
    )
    .unwrap();
}

#[test]
fn decode_encode_roundtrip_compressed() {
    walk_path(
        &Path::new("tests/data").to_path_buf(),
        &mut |path: &PathBuf| {
            do_roudtrip_with_encoder_options(
                path,
                EncoderOptions {
                    endianness: Endianness::BigEndian,
                    header_option: HeaderOption::Compressed(3),
                },
            )
        },
    )
    .unwrap();
}

struct EncoderOptions {
    endianness: Endianness,
    header_option: HeaderOption,
}

fn walk_path<F>(path: &PathBuf, f: &mut F) -> Result<(), Box<dyn Error>>
where
    F: FnMut(&PathBuf) -> Result<(), Box<dyn Error>>,
{
    let dir = fs::read_dir(path)?;

    for entry in dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_path(&path, f)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "fit") {
            if let Err(err) = f(&path) {
                return Err(format!("path: {:?}, err: {:?}", path, err).into());
            };
        }
    }
    Ok(())
}

fn do_roudtrip_with_encoder_options(
    path: &PathBuf,
    encoder_options: EncoderOptions,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(path).unwrap();
    let br = BufReader::new(file);
    let mut dec = Decoder::new(br);
    let buf = Vec::<u8>::with_capacity(5_000 >> 10); // 5 MB, large enough to avoid realloc.
    let mut cursor = Cursor::new(buf);

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
            return Err(format!("decode: {:?}", err).into());
        }
    } {
        cursor.seek(SeekFrom::Start(0)).unwrap();

        let mut enc = EncoderBuilder::new(&mut cursor)
            .endianness(encoder_options.endianness)
            .header_option(encoder_options.header_option)
            .build();

        let expected_messages = fit.messages.clone(); // Must clone since encoder mutates the value.

        if let Err(err) = enc.encode(fit) {
            return Err(format!("encode: {:?}", err).into());
        }

        cursor.seek(SeekFrom::Start(0)).unwrap();

        let mut dec = Decoder::new(&mut cursor);
        let result_fit = match dec.decode() {
            Ok(result_fit) => result_fit,
            Err(err) => {
                return Err(format!("re-decode the encoded file: {:?}", err).into());
            }
        };

        let result_messages = result_fit.unwrap().messages;

        if result_messages.len() == 0 || result_messages.len() != expected_messages.len() {
            return Err(format!(
                "unexpected messages len, expected: {}, got: {}",
                expected_messages.len(),
                result_messages.len()
            )
            .into());
        }

        for (i, mesg) in expected_messages
            .iter()
            .zip(result_messages.iter())
            .enumerate()
        {
            if mesg.0.num != mesg.1.num {
                return Err(format!(
                    "mesg num mismatch for mesg index {}, expected: {}, got: {}",
                    i, mesg.0.num, mesg.1.num
                )
                .into());
            }

            if mesg.0.fields.len() != mesg.1.fields.len() {
                return Err(format!(
                    "fields len mismatch for mesg index {} num {}, expected: {}, got: {}",
                    i,
                    mesg.0.num,
                    mesg.0.fields.len(),
                    mesg.1.fields.len()
                )
                .into());
            }
        }
    }
    Ok(())
}
