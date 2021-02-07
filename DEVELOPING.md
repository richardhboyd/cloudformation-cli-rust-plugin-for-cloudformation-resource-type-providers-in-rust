# Development

## Setup

I use an Amazon Linux 2 Cloud9 Environment so that I can run AL2 natively and I don't have to fight
with `x86_64-unknown-linux-musl`. The high-level overview of how this works so far is:

- Create a new Cloud9 Environmnet
- Create a folder named `RustFormation` (this will be where we write our Rust code)
- Create a folder named `ScratchProvider` (this will be the scratchpad for generating an rpdk template)
- Create a folder named `extract` (This will be where we do our zip wizardry)
- Build the generated ScratchProvider resource with `cfn submit --dry-run`
- Unzip the built zip file into the `extract` folder
- Zip the compiled Rust provider and place it in the `extract` folder and name it `ResourceProvider.zip`
- Zip the contents of `extract`
- Upload zip file to S3
- Register Type with AWS CLI

## Creating the Rust Binary

```
cargo new rust_formation --bin
```

Add the following to the created file `rust_formation/Cargo.toml`

```
[dependencies]
lambda_runtime = "0.1"
serde = "1.0.82"
serde_derive = "1.0.82"
serde_json = "1.0.33"

[[bin]]
name = "bootstrap"
path = "src/main.rs"
```

Replace the contents of `rust_formation/src/main.rs` with the following

```rust
use lambda_runtime::{error::HandlerError, lambda};
use std::error::Error;
use serde_derive::{Serialize, Deserialize};

#[derive(Deserialize, Clone)]
struct CustomEvent {
    #[serde(rename = "queryStringParameters")]
    query_string_parameters: Option<QueryString>,
    body: Option<String>,
}

#[derive(Deserialize, Clone)]
struct QueryString {
    #[serde(rename = "firstName")]
    first_name: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Body {
    #[serde(rename = "firstName")]
    first_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ResourceModel {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "TestCode")]
    test_code: String,
    #[serde(rename = "TPSCode")]
    tps_code: String
}

#[derive(Serialize, Clone)]
struct CustomOutput {
    message: String,
    #[serde(rename = "resourceModel")]
    model: ResourceModel,
    status: String
}

impl CustomOutput {
    fn new(message: String, model: ResourceModel, status: String) -> Self {
        CustomOutput {
            message, model, status
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    lambda!(my_handler);
    Ok(())
}

fn my_handler(e: CustomEvent, c: lambda_runtime::Context) -> Result<CustomOutput, HandlerError> {
    Ok(CustomOutput {
        message: format!("Hello! Welcome to the rust program in Lambda Function. Please add parameters."),
        model: ResourceModel {
            title: format!("111111111111111111111"),
            test_code: format!("CANCELLED"),
            tps_code: format!("AAAA00000000-0000")
        },
        status: format!("SUCCESS")
    })
}
```

build the binary with 
```
cd rust_formation
cargo build --release
zip -j rust.zip ./target/release/bootstrap
```


## Useful Links

- [AWS Lambda Rust Runtime](https://github.com/awslabs/aws-lambda-rust-runtime)
- [Helpful blog](https://blog.knoldus.com/aws-lambda-with-rust/)