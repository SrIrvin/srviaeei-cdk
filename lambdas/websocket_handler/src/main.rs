use aws_lambda_events::apigw::{ApiGatewayProxyResponse, ApiGatewayWebsocketProxyRequest};
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamodbClient;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    let config = aws_config::load_from_env().await;
    let dynamodb_client = DynamodbClient::new(&config);
    let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "SrviaeeiConnections".to_string());

    let dynamodb_client_ref = &dynamodb_client;
    let table_name_ref = &table_name;

    run(service_fn(move |event: LambdaEvent<ApiGatewayWebsocketProxyRequest>| async move {
        handler(dynamodb_client_ref, table_name_ref, event).await
    }))
    .await
}

async fn handler(
    db_client: &DynamodbClient,
    table_name: &str,
    event: LambdaEvent<ApiGatewayWebsocketProxyRequest>,
) -> Result<ApiGatewayProxyResponse, Error> {
    let request = event.payload;
    let route_key = request.request_context.route_key.as_deref().unwrap_or("");
    let connection_id = request.request_context.connection_id.as_deref().unwrap_or("");
    
    // Parse studentId (or userId) from the connection query parameters, fallback to default
    let query_params = &request.query_string_parameters;
    let student_id = query_params
        .first("studentId")
        .or_else(|| query_params.first("userId"))
        .unwrap_or("default_student")
        .to_string();

    tracing::info!(
        "Route Key: {}, Connection ID: {}, Student ID: {}",
        route_key,
        connection_id,
        student_id
    );

    if connection_id.is_empty() {
        return Ok(ApiGatewayProxyResponse {
            status_code: 400,
            body: Some("Missing connectionId".into()),
            is_base64_encoded: false,
            headers: Default::default(),
            multi_value_headers: Default::default(),
        });
    }

    match route_key {
        "$connect" => {
            tracing::info!("Handling $connect for student: {}", student_id);
            db_client
                .put_item()
                .table_name(table_name)
                .item("connectionId", AttributeValue::S(connection_id.to_string()))
                .item("studentId", AttributeValue::S(student_id))
                .item(
                    "connectedAt",
                    AttributeValue::N(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            .to_string(),
                    ),
                )
                .send()
                .await?;
        }
        "$disconnect" => {
            tracing::info!("Handling $disconnect for connection: {}", connection_id);
            db_client
                .delete_item()
                .table_name(table_name)
                .key("connectionId", AttributeValue::S(connection_id.to_string()))
                .send()
                .await?;
        }
        _ => {
            // Default route or custom messages
            let body_content = request.body.as_deref().unwrap_or("");
            tracing::info!(
                "WebSocket Default Route - Received message from student {}: {}",
                student_id,
                body_content
            );
        }
    }

    Ok(ApiGatewayProxyResponse {
        status_code: 200,
        body: Some("Success".into()),
        is_base64_encoded: false,
        headers: Default::default(),
        multi_value_headers: Default::default(),
    })
}
