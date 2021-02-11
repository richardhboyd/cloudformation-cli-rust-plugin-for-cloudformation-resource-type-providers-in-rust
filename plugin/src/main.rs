use chrono::Utc;
use lambda::{handler_fn};
use serde_derive::{Serialize, Deserialize};
use std::fmt::Debug;
use std::collections::HashMap;
use rusoto_signature::region::Region as SRegion;
use std::{str};
pub type Error = Box<dyn std::error::Error + Send + Sync + 'static>;
use rusoto_credential::StaticProvider;
use rusoto_logs::{
    CloudWatchLogs, CloudWatchLogsClient, CreateLogGroupRequest, CreateLogStreamRequest, DescribeLogStreamsRequest, InputLogEvent,
    PutLogEventsRequest,
};
use std::default::Default;
// David's Sigv4 example
use aws_sigv4::{sign, Credentials as Sigv4Credentials};
use bytes::Bytes;
use http::{header, Method, Request, Uri, Version};
use http_body::Body as _;
use hyper::{Body, Client};

fn reconstruct(req: Request<Bytes>) -> Request<Body> {
    let (headers, body) = req.into_parts();
    let body = Body::from(body);
    Request::from_parts(headers, body)
}

async fn call_cloud_9(access_key: &str, secret_key: &str, security_token: &str, owner_arn: &str) -> Result<(), Error> {
    let https = hyper_tls::HttpsConnector::new();
    let client: Client<_, hyper::Body> = Client::builder().build(https);
    let uri =
        Uri::from_static("https://cloud9.us-west-2.amazonaws.com/");
    let builder = Request::builder();
    let mut builder = builder
        .method(Method::POST)
        .uri(uri)
        .version(Version::HTTP_11);
    let headers = builder.headers_mut().expect("Missing headers");
    let hdr = header::HeaderName::from_lowercase(b"x-amz-target").unwrap();
    headers.insert(header::HOST, "cloud9.amazonaws.com".parse()?);
    headers.insert(header::CONTENT_TYPE, "application/x-amz-json-1.1".parse()?);
    headers.insert(hdr, "AWSCloud9WorkspaceManagementService.CreateEnvironmentEC2".parse()?);
    let s: String = format!("{{
            \"name\":\"dddd\",
            \"automaticStopTimeMinutes\":30,
            \"ownerArn\": \"{}\",
            \"clientRequestToken\": \"cloud9-console-a2450903-b63e-4033-ac0e-d15519af0d57\",
            \"instanceType\":\"t3.small\",
            \"ideTemplateId\":\"f5ec09dc16f0a23728e3cfee668658e8\"
        }}", owner_arn).to_owned();
    let s_slice: &str = &s[..];  // take a full slice of the string
    let src3: &str = "{
            \"name\":\"dddd\",
            \"automaticStopTimeMinutes\":30,
            \"ownerArn\": \"arn:aws:sts::026781393487:assumed-role/newRichardRole/rhboyd-Isengard\",
            \"clientRequestToken\": \"cloud9-console-a2450903-b63e-4033-ac0e-d15519af0d57\",
            \"instanceType\":\"t3.small\",
            \"ideTemplateId\":\"f5ec09dc16f0a23728e3cfee668658e8\"
        }";
    let mut req = builder.body(Bytes::from(src3.as_bytes()))?;
    let credentials = Sigv4Credentials {
        access_key: access_key.to_string(),
        secret_key: secret_key.to_string(),
        security_token: Some(security_token.to_string())
    };

    sign(&mut req, &credentials, "us-west-2", "cloud9")?;
    let req = reconstruct(req);
    let mut res = client.request(req).await?;
    let mut body = vec![];
    while let Some(Ok(chunk)) = res.body_mut().data().await {
        body.extend_from_slice(&chunk);
    }
    let s = match str::from_utf8(&body) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };

    println!("result: {}", s);
    // println!("{:?}", body);
    

    Ok(())
}

async fn do_step_1(client: CloudWatchLogsClient, log_group_name: &str) -> Result<(), Error> {
    let mut create_log_group_req: CreateLogGroupRequest = Default::default();
    create_log_group_req.log_group_name = log_group_name.to_string();
    let streams_resp = client.create_log_group(create_log_group_req).await?;
    Ok(())
}
async fn do_step_2(client: CloudWatchLogsClient, log_group_name: &str, log_stream_name: &str) -> Result<(), Error> {
    let mut create_log_stream_req: CreateLogStreamRequest = Default::default();
    create_log_stream_req.log_group_name = log_group_name.to_string();
    create_log_stream_req.log_stream_name = log_stream_name.to_string();
    let streams_resp = client.create_log_stream(create_log_stream_req).await?;
    Ok(())
}

async fn send_to_cloudwatch(
    provider: StaticProvider,
    log_group_name: &str,
    log_stream_name: &str,
    contents: &str
    )  -> Result<(), Error> {

    let client = CloudWatchLogsClient::new_with(
        rusoto_core::request::HttpClient::new().expect("failed to create request dispatcher"),
        provider,
        SRegion::UsWest2
    );
    
    if let Ok(ret) = do_step_1(client.clone(), &log_group_name).await {
      println!("Created new Log Group")
    } else {
        println!("Log Group {} already exists", &log_group_name)
    }
    
    if let Ok(ret) = do_step_2(client.clone(), &log_group_name, &log_stream_name).await {
      println!("Created new Log Stream")
    } else {
      println!("Log Stream {} already exists", &log_stream_name)
    }
    
 
    let input_log_event = InputLogEvent {
        message: contents.to_string(),
        timestamp: Utc::now().timestamp_millis(),
    };
 
    // We need the log stream to get the sequence token
    let mut desc_streams_req: DescribeLogStreamsRequest = Default::default();
    desc_streams_req.log_group_name = log_group_name.to_string();
    desc_streams_req.log_stream_name_prefix = Some(log_stream_name.to_string());
    let streams_resp = client.describe_log_streams(desc_streams_req).await?;
    let log_streams = streams_resp.log_streams.unwrap();
    let iter = log_streams.iter();
    let sequence_token = if iter.count() > 0 {
        let stream = &log_streams
          .iter()
          .find(|s| s.log_stream_name == Some(log_stream_name.to_string()))
          .unwrap();
        stream.upload_sequence_token.clone()
    } else {
        None
    };
 
    let put_log_events_request = PutLogEventsRequest {
        log_events: vec![input_log_event], // > 1 must sort by timestamp ASC
        log_group_name: log_group_name.to_string(),
        log_stream_name: log_stream_name.to_string(),
        sequence_token,
    };
 
    let resp = client.put_log_events(put_log_events_request).await?;
    Ok(())
}
//

#[derive(Deserialize, Clone, Debug)]
struct CustomEvent {
    action: String,
    #[serde(rename = "awsAccountId")]
    aws_account_id: String,
    #[serde(rename = "bearerToken")]
    bearer_token: String,
    region: String,
    #[serde(rename = "responseEndpoint")]
    response_endpoint: Option<String>,
    #[serde(rename = "nextToken")]
    next_token: Option<String>,
    #[serde(rename = "resourceType")]
    resource_type: String,
    #[serde(rename = "resourceTypeVersion")]
    resource_type_version: String,
    #[serde(rename = "requestData")]
    request_data: RequestData,
    #[serde(rename = "stackId")]
    stack_id: String,
    #[serde(rename = "callbackContext")]
    callback_context: serde_json::Value,
    #[serde(rename = "snapshotRequested")]
    snapshot_requested: Option<bool>,
    rollback: Option<bool>,
    driftable: Option<bool>
}

#[derive(Deserialize, Clone, Debug)]
struct RequestData {
    #[serde(rename = "callerCredentials")]
    caller_credentials: Credentials,
    #[serde(rename = "providerCredentials")]
    provider_credentials: Credentials,
    #[serde(rename = "providerLogGroupName")]
    provider_log_group_name: String,
    #[serde(rename = "logicalResourceId")]
    logical_resource_id: String,
    #[serde(rename = "resourceProperties")]
    resource_properties: ResourceModel,
    #[serde(rename = "previousResourceProperties")]
    previous_resource_properties: Option<ResourceModel>,
    #[serde(rename = "systemTags")]
    system_tags: Option<HashMap<String, String>>,
    #[serde(rename = "previousSystemTags")]
    previous_system_tags: Option<HashMap<String, String>>,
    #[serde(rename = "stackTags")]
    stack_tags: Option<HashMap<String, String>>,
    #[serde(rename = "previousStackTags")]
    previous_stack_tags: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Clone, Debug)]
struct Credentials {
    #[serde(rename = "accessKeyId")]
    access_key_id: String,
    #[serde(rename = "secretAccessKey")]
    secret_access_key: String,
    #[serde(rename = "sessionToken")]
    session_token: String
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct ResourceModel {
    #[serde(rename = "EnvironmentOwner")]
    environment_owner: Option<String>,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "InstanceId")]
    instance_id: String,
    #[serde(rename = "Username")]
    #[serde(default = "default_username")]
    username: String,
    #[serde(rename = "EnvironmentName")]
    environment_name: Option<String>,
    #[serde(rename = "EnvironmentId")]
    environment_id: Option<String>,
    #[serde(rename = "Environment")]
    environment_return_value: Option<String>,
    #[serde(rename = "Arn")]
    arn: Option<String>,
    #[serde(rename = "NodeBinaryPath")]
    #[serde(default = "default_node_binary_path")]
    node_binary_path: String,
    #[serde(rename = "EnvironmentPath")]
    #[serde(default = "default_environment_path")]
    environment_path: String,
}

fn default_username() -> String {
    String::from("ec2-user")
}

fn default_node_binary_path() -> String {
    String::from("/usr/bin/node")
}

fn default_environment_path() -> String {
    String::from("~/environment")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Collaborator {
    #[serde(rename = "Arn")]
    arn: String,
    #[serde(rename = "Permissions")]
    permissions: String
}

#[derive(Serialize, Clone, Debug)]
struct CustomOutput {
    status: String,
    // #[serde(rename = "errorCode")]
    // error_code: String,
    message: String,
    // #[serde(rename = "callbackContext")]
    // callback_context: String, //TODO make this a struct
    // #[serde(rename = "callbackDelaySeconds")]
    // callback_delay_seconds: i32,
    #[serde(rename = "resourceModel")]
    resource_model: ResourceModel
    // #[serde(rename = "resourceModels")]
    // resource_models: ResourceModel,
    // #[serde(rename = "nextToken")]
    // next_token: String
    
}

impl CustomOutput {
    fn new(message: String, resource_model: ResourceModel, status: String) -> Self {
        CustomOutput {
            message, resource_model, status
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = handler_fn(my_handler);
    lambda::run(func).await?;
    Ok(())
}

async fn my_handler(e: CustomEvent, _c: lambda::Context) -> Result<CustomOutput, Error> {
    let access_key_id: String = e.request_data.provider_credentials.access_key_id.clone();
    let secret_access_key: String = e.request_data.provider_credentials.secret_access_key.clone();
    let session_token: String = e.request_data.provider_credentials.session_token.clone();
    let log_group_name: String = e.request_data.provider_log_group_name.clone();
    // DANGER!!! This will fail if OwnerArn is not defined.
    let owner_arn: String = e.request_data.resource_properties.environment_owner.clone().unwrap();
    let stack_id = e.stack_id.clone();
    let mut v: Vec<&str> = stack_id.split(':').collect();
    let log_stream_name: &str = v.pop().unwrap();
    let none: Option<i64> = None;
    let provider = StaticProvider::new(access_key_id.clone(), secret_access_key.clone(), Some(session_token.clone()), none);
    let input = String::from(e.action.clone());
    match input {
        _ if input == "CREATE" => {
            send_to_cloudwatch(provider, &log_group_name, &log_stream_name, &String::from(format!("{:?}", e))).await?;
            call_cloud_9(&access_key_id, &secret_access_key, &session_token, &owner_arn).await?;
        },
        _ if input == "DELETE" => {
            println!("DELETE Event");
        },
        _ => println!("Input does not equal any value"),
    }
    

    Ok(CustomOutput {
        message: format!("SUCCESS"),
        resource_model: ResourceModel {
            instance_id: format!("i-XXXXXXXXXX"),
            arn: Some(format!("arn:aws:cloud9:us-west-2:000000000000:environment/myEnv")),
            environment_id: Some(format!("AAAA00000000-0000")),
            environment_return_value: Some(format!("AAAA00000000-0000")),
            ..Default::default()
        },
        status: format!("SUCCESS")
    })
}