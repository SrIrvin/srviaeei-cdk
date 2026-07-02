import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as dynamodb from 'aws-cdk-lib/aws-dynamodb';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as path from 'path';
import { WebSocketApi, WebSocketStage } from '@aws-cdk/aws-apigatewayv2-alpha';
import { WebSocketLambdaIntegration } from '@aws-cdk/aws-apigatewayv2-integrations-alpha';

export class SrviaeeiStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // 1. DynamoDB Table to store connection state
    const connectionsTable = new dynamodb.Table(this, 'SrviaeeiConnections', {
      partitionKey: { name: 'connectionId', type: dynamodb.AttributeType.STRING },
      tableName: 'SrviaeeiConnections',
      billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
      removalPolicy: cdk.RemovalPolicy.DESTROY, // Dev removal policy
    });

    // 2. Rust Lambda Function for WebSocket handling (using precompiled binary)
    const webSocketHandler = new lambda.Function(this, 'WebSocketHandlerLambda', {
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap', // Required but not used by custom runtime
      code: lambda.Code.fromAsset(path.join(__dirname, '../lambdas/target/lambda/websocket_handler')),
      timeout: cdk.Duration.seconds(15),
      architecture: lambda.Architecture.ARM_64,
      environment: {
        TABLE_NAME: connectionsTable.tableName,
        RUST_LOG: 'info',
      },
    });

    // Grant permissions on DynamoDB
    connectionsTable.grantReadWriteData(webSocketHandler);

    // 3. WebSocket API Gateway
    const webSocketApi = new WebSocketApi(this, 'SrviaeeiWebSocketApi', {
      apiName: 'SrviaeeiWebSocketApi',
    });

    // Integrations
    const socketIntegration = new WebSocketLambdaIntegration('SocketIntegration', webSocketHandler);

    // Routes
    webSocketApi.addRoute('$connect', {
      integration: socketIntegration,
    });

    webSocketApi.addRoute('$disconnect', {
      integration: socketIntegration,
    });

    webSocketApi.addRoute('$default', {
      integration: socketIntegration,
    });

    // 4. WebSocket Stage
    const devStage = new WebSocketStage(this, 'DevStage', {
      webSocketApi,
      stageName: 'dev',
      autoDeploy: true,
    });

    // 5. Grant API Gateway Connection Management permissions to the lambda
    // This allows pushing messages downstream if needed (e.g. notifications/affective loop response)
    const connectionsArn = `arn:aws:execute-api:${this.region}:${this.account}:${webSocketApi.apiId}/${devStage.stageName}/POST/@connections/*`;
    webSocketHandler.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['execute-api:ManageConnections'],
      resources: [connectionsArn],
    }));

    // Output the WebSocket URL
    new cdk.CfnOutput(this, 'WebSocketUrl', {
      value: devStage.callbackUrl,
      description: 'The WebSocket URL of the SRVIAEEI API',
    });
  }
}
