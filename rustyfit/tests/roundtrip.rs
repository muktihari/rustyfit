use embedded_io_adapters::std::FromStd;
use rustyfit::{
    Decoder, DecoderEvent, Encoder, EncoderBuilder, Endianness, HeaderOption, StreamingIterator,
    proto::Message,
};
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, Cursor, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[test]
fn decode_encode_roundtrip() {
    walk_path(
        &Path::new("tests/data").to_path_buf(),
        &mut |path: &PathBuf| {
            do_roudtrip_with_encoder_options(
                path,
                EncoderBuilder::new()
                    .endianness(Endianness::LittleEndian)
                    .header_option(HeaderOption::Normal(0)),
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
                EncoderBuilder::new()
                    .endianness(Endianness::BigEndian)
                    .header_option(HeaderOption::Compressed(3)),
            )
        },
    )
    .unwrap();
}

#[test]
fn streaming_decode_encode_roundtrip() {
    walk_path(
        &Path::new("tests/data").to_path_buf(),
        &mut |path: &PathBuf| do_roudtrip_by_streaming(path),
    )
    .unwrap();
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
        if path.extension().is_some_and(|ext| ext == "fit")
            && let Err(err) = f(&path)
        {
            return Err(format!("path: {:?}, err: {:?}", path, err).into());
        }
    }
    Ok(())
}

fn do_roudtrip_with_encoder_options(
    path: &PathBuf,
    encoder_builder: EncoderBuilder,
) -> Result<(), Box<dyn Error>> {
    let mut dec = Decoder::new();

    let file = File::open(path).unwrap();
    let br = BufReader::new(file);
    let mut reader = FromStd::new(br);

    let buf = Vec::<u8>::with_capacity(5000 * 1024); // 5 MB, large enough to avoid realloc.
    let mut cursor = Cursor::new(buf);

    while let Some(fit) = &mut dec.decode(&mut reader)? {
        cursor.seek(SeekFrom::Start(0)).unwrap();

        let mut enc = encoder_builder.build();

        let expected_messages = fit.messages.clone(); // Must clone since encoder mutates the value.

        let mut writer = FromStd::new(&mut cursor);
        if let Err(err) = enc.encode(&mut writer, fit) {
            return Err(format!("encode: {:?}", err).into());
        }

        cursor.seek(SeekFrom::Start(0)).unwrap();

        let mut dec = Decoder::new();
        let result_fit = match dec.decode(&mut FromStd::new(&mut cursor)) {
            Ok(result_fit) => result_fit,
            Err(err) => {
                return Err(format!("re-decode the encoded file: {:?}", err).into());
            }
        };

        let result_messages = result_fit.unwrap().messages;

        if result_messages.is_empty() || result_messages.len() != expected_messages.len() {
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

fn do_roudtrip_by_streaming(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if let Some(file_name) = path.file_name()
        && ["WeightScaleMultiUser.fit", "Settings.fit"]
            .iter()
            .any(|x| *x == file_name)
    {
        return Ok(());
    }

    let file = File::open(path).unwrap();
    let br = BufReader::new(file);
    let mut reader = FromStd::new(br);

    let buf = Vec::<u8>::with_capacity(5000 * 1024); // 5 MB, large enough to avoid realloc.
    let mut cursor = Cursor::new(buf);

    let mut dec = Decoder::new();
    let mut stream_dec = dec.stream(&mut reader);

    let mut expected_messages = Vec::<Message>::new();
    while let Some(event) = stream_dec.next() {
        if let DecoderEvent::Message(v) = event? {
            expected_messages.push(v.clone());
        }
    }

    cursor.seek(SeekFrom::Start(0)).unwrap();
    let mut writer = FromStd::new(&mut cursor);

    let mut enc = Encoder::new();
    let mut stream_enc = enc.stream(&mut writer);

    for mesg in expected_messages.clone().iter_mut() {
        stream_enc.write_message(mesg)?;
    }
    stream_enc.finish()?;

    cursor.seek(SeekFrom::Start(0)).unwrap();
    let mut reader = FromStd::new(&mut cursor);

    let mut dec = Decoder::new();
    let mut stream_dec = dec.stream(&mut reader);

    let mut result_messages = Vec::<Message>::new();
    while let Some(event) = stream_dec.next() {
        if let DecoderEvent::Message(v) = event? {
            result_messages.push(v.clone())
        }
    }

    if result_messages.is_empty() || result_messages.len() != expected_messages.len() {
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

    Ok(())
}
