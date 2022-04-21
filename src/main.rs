use clap::{Parser, Subcommand};
use ring::{rand, signature};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::Read;

const BUF_SIZE: usize = 1_000_000;

#[derive(Subcommand, Clone)]
enum Command {
    Sign(SignCommand),
    Sha256(Sha256Command),
    Verify(VerifyCommand),
}

#[derive(Parser, Clone)]
struct Sha256Command {
    #[clap()]
    file_path: String,
}

#[derive(Parser, Clone)]
struct SignCommand {
    #[clap()]
    file_path: String,
    #[clap()]
    private_key: String,
}

#[derive(Parser, Clone)]
struct VerifyCommand {
    #[clap()]
    file_path: String,
    #[clap()]
    public_key: String,
    #[clap()]
    signature: String,
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Opts {
    #[clap(short = 'J', long = "json-output")]
    json: bool,
    #[clap(subcommand)]
    command: Command,
}

enum ErrorKind {
    Io,
    Decode,
    Rsa,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorKind::Io => "I/O error",
                ErrorKind::Decode => "Decode error",
                ErrorKind::Rsa => "RSA error",
            }
        )
    }
}

struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    fn io(msg: impl fmt::Display) -> Self {
        Self {
            kind: ErrorKind::Io,
            message: msg.to_string(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error {
            kind: ErrorKind::Io,
            message: e.to_string(),
        }
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Error {
        Error {
            kind: ErrorKind::Decode,
            message: e.to_string(),
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Error {
        Error {
            kind: ErrorKind::Decode,
            message: e.to_string(),
        }
    }
}

impl From<ring::error::KeyRejected> for Error {
    fn from(e: ring::error::KeyRejected) -> Error {
        Error {
            kind: ErrorKind::Rsa,
            message: e.to_string(),
        }
    }
}

impl From<ring::error::Unspecified> for Error {
    fn from(e: ring::error::Unspecified) -> Error {
        Error {
            kind: ErrorKind::Rsa,
            message: e.to_string(),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

type EResult<T> = Result<T, Error>;

fn sign(file_path: &str, pvt_key_path: &str) -> EResult<(Vec<u8>, Vec<u8>)> {
    let key = read_file(pvt_key_path)
        .map_err(|e| Error::io(format!("Unable to read file {}: {}", pvt_key_path, e)))?;
    let key_pair = signature::RsaKeyPair::from_der(&key)?;
    let content = file_sha256(file_path)
        .map_err(|e| Error::io(format!("Unable to read file {}: {}", file_path, e)))?;
    let rng = rand::SystemRandom::new();
    let mut signature = vec![0; key_pair.public_modulus_len()];
    key_pair.sign(&signature::RSA_PKCS1_SHA256, &rng, &content, &mut signature)?;
    Ok((signature, content))
}

fn verify(file_path: &str, pub_key_path: &str, signature: &[u8]) -> EResult<()> {
    let key = read_file(pub_key_path)
        .map_err(|e| Error::io(format!("Unable to read file {}: {}", pub_key_path, e)))?;
    let content = file_sha256(file_path)
        .map_err(|e| Error::io(format!("Unable to read file {}: {}", file_path, e)))?;
    let k = signature::UnparsedPublicKey::new(&signature::RSA_PKCS1_2048_8192_SHA256, key);
    k.verify(&content, signature).map_err(Into::into)
}

fn read_file(path: &str) -> Result<Vec<u8>, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut buf: Vec<u8> = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn file_sha256(path: &str) -> Result<Vec<u8>, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0; BUF_SIZE];
    let mut hasher = Sha256::new();
    loop {
        let r = file.read(&mut buf)?;
        if r == BUF_SIZE {
            hasher.update(&buf);
        } else {
            hasher.update(&buf[..r]);
            break;
        }
    }
    Ok(hasher.finalize().to_vec())
}

fn main() -> EResult<()> {
    let opts = Opts::parse();
    match opts.command {
        Command::Sign(c) => {
            let (sig, sha256sum) = sign(&c.file_path, &c.private_key)?;
            let signature = base64::encode(sig);
            if opts.json {
                println!(
                    r#"{{"sha256":"{}","signature":"{}"}}"#,
                    hex::encode(sha256sum),
                    signature
                );
            } else {
                println!("{}", signature);
            }
        }
        Command::Verify(c) => {
            let sig = base64::decode(c.signature)?;
            verify(&c.file_path, &c.public_key, &sig)?;
            if opts.json {
                println!(r#"{{"ok":true}}"#);
            } else {
                println!("signature valid");
            }
        }
        Command::Sha256(c) => {
            let sha256sum = hex::encode(file_sha256(&c.file_path)?);
            if opts.json {
                println!(r#"{{"sha256":"{}"}}"#, sha256sum);
            } else {
                println!("{}", sha256sum);
            }
        }
    }
    Ok(())
}
