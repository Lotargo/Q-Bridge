use arrow_flight::flight_service_server::{FlightService, FlightServiceServer};
use arrow_flight::{
    Action, ActionType, Criteria, Empty, FlightData, FlightDescriptor, FlightInfo,
    HandshakeRequest, HandshakeResponse, PollInfo, PutResult, SchemaResult, Ticket,
};
use async_trait::async_trait;
use futures::stream::{self, Stream};
use std::pin::Pin;
use tonic::{transport::Server, Request, Response, Status, Streaming};

type FlightStream<T> = Pin<Box<dyn Stream<Item = T> + Send + Sync + 'static>>;

#[derive(Clone, Default)]
pub struct MyFlightService {}

#[async_trait]
impl FlightService for MyFlightService {
    type DoGetStream = FlightStream<Result<FlightData, Status>>;
    type DoActionStream = FlightStream<Result<arrow_flight::Result, Status>>;
    type DoExchangeStream = FlightStream<Result<FlightData, Status>>;
    type DoPutStream = FlightStream<Result<PutResult, Status>>;
    type HandshakeStream = FlightStream<Result<HandshakeResponse, Status>>;
    type ListActionsStream = FlightStream<Result<ActionType, Status>>;
    type ListFlightsStream = FlightStream<Result<FlightInfo, Status>>;

    async fn do_get(
        &self,
        _request: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        println!("Received do_get request, returning empty stream as a placeholder.");
        let stream = stream::empty();
        Ok(Response::new(Box::pin(stream) as Self::DoGetStream))
    }

    // --- Default implementations for other methods ---
    async fn handshake(
        &self,
        _request: Request<Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        Err(Status::unimplemented("handshake"))
    }
    async fn list_flights(
        &self,
        _request: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        Err(Status::unimplemented("list_flights"))
    }
    async fn get_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        Err(Status::unimplemented("get_flight_info"))
    }
    async fn poll_flight_info(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<PollInfo>, Status> {
        Err(Status::unimplemented("poll_flight_info"))
    }
    async fn get_schema(
        &self,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        Err(Status::unimplemented("get_schema"))
    }
    async fn do_put(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        Err(Status::unimplemented("do_put"))
    }
    async fn do_exchange(
        &self,
        _request: Request<Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        Err(Status::unimplemented("do_exchange"))
    }
    async fn do_action(
        &self,
        _request: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        Err(Status::unimplemented("do_action"))
    }
    async fn list_actions(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        Err(Status::unimplemented("list_actions"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50052".parse()?;
    let service = MyFlightService::default();
    let server = FlightServiceServer::new(service);

    println!("Transport server listening on {}", addr);

    Server::builder().add_service(server).serve(addr).await?;

    Ok(())
}
