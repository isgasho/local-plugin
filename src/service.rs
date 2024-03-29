use crate::database::establish_connection;
use crate::diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use crate::models::{QueryableList, QueryableTask};
use crate::schema::lists::dsl::*;
use crate::schema::tasks::dsl::*;
use anyhow::Context;
use proto_rust::provider::provider_server::Provider;
use proto_rust::provider::{CountResponse, Empty, List, ListResponse, Task, TaskResponse};
use proto_rust::{ListIdResponse, TaskIdResponse};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

#[derive(Debug, Default)]
pub struct LocalService {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
}

#[tonic::async_trait]
impl Provider for LocalService {
    async fn get_id(&self, request: Request<Empty>) -> Result<Response<String>, Status> {
        tracing::info!("Request received: {request:?}");
        Ok(Response::new(self.id.clone()))
    }

    async fn get_name(&self, request: Request<Empty>) -> Result<Response<String>, Status> {
        tracing::info!("Request received: {request:?}");
        Ok(Response::new(self.name.clone()))
    }

    async fn get_description(&self, request: Request<Empty>) -> Result<Response<String>, Status> {
        tracing::info!("Request received: {request:?}");
        Ok(Response::new(self.description.clone()))
    }

    async fn get_icon_name(&self, request: Request<Empty>) -> Result<Response<String>, Status> {
        tracing::info!("Request received: {request:?}");
        Ok(Response::new(self.icon.clone()))
    }

    type ReadAllTasksStream = ReceiverStream<Result<TaskResponse, Status>>;

    async fn read_all_tasks(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<Self::ReadAllTasksStream>, Status> {
        tracing::info!("Request received: {request:?}");
        let (tx, rx) = tokio::sync::mpsc::channel(4);

        let send_request = || -> anyhow::Result<Vec<Task>> {
            let result: Vec<QueryableTask> = tasks
                .load::<QueryableTask>(&mut establish_connection()?)
                .context("Failed to fetch list of tasks.")?;
            let results: Vec<Task> = result.iter().map(|t| t.clone().into()).collect();
            Ok(results)
        };

        let mut response = TaskResponse::default();

        tokio::spawn(async move {
            match send_request() {
                Ok(value) => {
                    response.successful = true;
                    for task in &value[..] {
                        let response = TaskResponse {
                            successful: true,
                            message: "Task fetched succesfully.".to_string(),
                            task: Some(task.clone()),
                        };
                        tx.send(Ok(response)).await.unwrap();
                    }
                }
                Err(err) => response.message = err.to_string(),
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type ReadTasksFromListStream = ReceiverStream<Result<TaskResponse, Status>>;

    async fn read_tasks_from_list(
        &self,
        request: Request<String>,
    ) -> Result<Response<Self::ReadTasksFromListStream>, Status> {
        tracing::info!("Request received: {request:?}");
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let id = request.into_inner();

        let send_request = || -> anyhow::Result<Vec<Task>> {
            let result: Vec<QueryableTask> = tasks
                .filter(parent_list.eq(id))
                .load::<QueryableTask>(&mut establish_connection()?)
                .context("Failed to fetch list of tasks.")?;
            let results: Vec<Task> = result.iter().map(|t| t.clone().into()).collect();
            Ok(results)
        };

        let mut response = TaskResponse::default();

        tokio::spawn(async move {
            match send_request() {
                Ok(value) => {
                    response.successful = true;
                    for task in &value[..] {
                        let response = TaskResponse {
                            successful: true,
                            message: "Task fetched successfully".to_string(),
                            task: Some(task.clone()),
                        };
                        tx.send(Ok(response)).await.unwrap();
                    }
                }
                Err(err) => response.message = err.to_string(),
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn read_task_ids_from_list(
        &self,
        request: Request<String>,
    ) -> Result<Response<TaskIdResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let send_request = || -> anyhow::Result<Vec<String>> {
            let result: Vec<String> = tasks
                .select(id_task)
                .filter(parent_list.eq(request.into_inner()))
                .load::<String>(&mut establish_connection()?)
                .context("Failed to fetch list of tasks.")?;
            Ok(result)
        };

        let mut response = TaskIdResponse {
            successful: true,
            message: String::new(),
            tasks: vec![],
        };

        match send_request() {
            Ok(result) => {
                response.successful = true;
                response.tasks = result;
            }
            Err(_) => response.message = "Failed to fetch list of tasks".to_string(),
        }

        Ok(Response::new(response))
    }

    async fn read_task_count_from_list(
        &self,
        request: Request<String>,
    ) -> Result<Response<CountResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let id = request.into_inner();
        let mut response = CountResponse::default();

        let send_request = || -> anyhow::Result<i64> {
            let count: i64 = tasks
                .filter(id_task.eq(id))
                .count()
                .get_result(&mut establish_connection()?)?;
            Ok(count)
        };

        match send_request() {
            Ok(value) => {
                response.count = value;
                response.successful = true;
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn create_task(&self, request: Request<Task>) -> Result<Response<TaskResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let task = request.into_inner();
        let mut response = TaskResponse::default();

        let send_request = || -> anyhow::Result<()> {
            let queryable_task: QueryableTask = task.clone().into();

            diesel::insert_into(tasks)
                .values(&queryable_task)
                .execute(&mut establish_connection()?)?;

            Ok(())
        };

        match send_request() {
            Ok(()) => {
                response.task = Some(task);
                response.successful = true;
                response.message = "Task added successfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn read_task(&self, request: Request<String>) -> Result<Response<TaskResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let id = request.into_inner();
        let mut response = TaskResponse::default();

        let send_request = || -> anyhow::Result<Task> {
            let result: QueryableTask = tasks
                .find(id)
                .first(&mut establish_connection()?)
                .context("Failed to fetch list of tasks.")?;
            Ok(result.into())
        };

        match send_request() {
            Ok(value) => {
                response.task = Some(value);
                response.successful = true;
                response.message = "Task fetched successfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn update_task(&self, request: Request<Task>) -> Result<Response<TaskResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let task = request.into_inner();
        let mut response = TaskResponse::default();

        let send_request = || -> anyhow::Result<()> {
            let task: QueryableTask = task.into();

            diesel::update(tasks.filter(id_task.eq(task.id_task.clone())))
                .set((
                    id_task.eq(task.id_task),
                    title.eq(task.title),
                    body.eq(task.body),
                    completed_on.eq(task.completed_on),
                    due_date.eq(task.due_date),
                    importance.eq(task.importance),
                    favorite.eq(task.favorite),
                    is_reminder_on.eq(task.is_reminder_on),
                    reminder_date.eq(task.reminder_date),
                    status.eq(task.status),
                    created_date_time.eq(task.created_date_time),
                    last_modified_date_time.eq(task.last_modified_date_time),
                ))
                .execute(&mut establish_connection()?)
                .context("Failed to update task.")?;

            Ok(())
        };

        match send_request() {
            Ok(()) => {
                response.task = None;
                response.successful = true;
                response.message = "Task updated successfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn delete_task(
        &self,
        request: Request<String>,
    ) -> Result<Response<TaskResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let id = request.into_inner();
        let mut response = TaskResponse::default();

        let send_request = || -> anyhow::Result<()> {
            diesel::delete(tasks.filter(id_task.eq(id))).execute(&mut establish_connection()?)?;

            Ok(())
        };

        match send_request() {
            Ok(()) => {
                response.task = None;
                response.successful = true;
                response.message = "Task removed successfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    type ReadAllListsStream = ReceiverStream<Result<ListResponse, Status>>;

    async fn read_all_lists(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<Self::ReadAllListsStream>, Status> {
        tracing::info!("Request received: {request:?}");
        let (tx, rx) = tokio::sync::mpsc::channel(4);

        let send_request = || -> anyhow::Result<Vec<List>> {
            let results = lists.load::<QueryableList>(&mut establish_connection()?)?;

            let results: Vec<List> = results.iter().map(|t| t.clone().into()).collect();
            Ok(results)
        };

        let mut response = ListResponse::default();

        tokio::spawn(async move {
            match send_request() {
                Ok(value) => {
                    response.successful = true;
                    for list in &value[..] {
                        let response = ListResponse {
                            successful: true,
                            message: "List fetched succesfully.".to_string(),
                            list: Some(list.clone()),
                        };
                        tx.send(Ok(response)).await.unwrap();
                    }
                }
                Err(err) => response.message = err.to_string(),
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn read_all_list_ids(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListIdResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let send_request = || -> anyhow::Result<Vec<String>> {
            let result: Vec<String> = lists
                .select(id_list)
                .load::<String>(&mut establish_connection()?)
                .context("Failed to fetch list of tasks.")?;
            Ok(result)
        };

        let mut response = ListIdResponse {
            successful: true,
            message: String::new(),
            lists: vec![],
        };

        match send_request() {
            Ok(result) => {
                response.successful = true;
                response.lists = result;
            }
            Err(_) => response.message = "Failed to fetch list of tasks".to_string(),
        }

        Ok(Response::new(response))
    }

    async fn create_list(&self, request: Request<List>) -> Result<Response<ListResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let list = request.into_inner();
        let mut response = ListResponse::default();

        let send_request = || -> anyhow::Result<()> {
            let list: QueryableList = list.into();

            diesel::insert_into(lists)
                .values(&list)
                .execute(&mut establish_connection()?)?;

            Ok(())
        };

        match send_request() {
            Ok(()) => {
                response.list = None;
                response.successful = true;
                response.message = "List added succesfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn read_list(&self, request: Request<String>) -> Result<Response<ListResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let id = request.into_inner();
        let mut response = ListResponse::default();

        let send_request = || -> anyhow::Result<List> {
            let result: QueryableList = lists.find(id).first(&mut establish_connection()?)?;
            Ok(result.into())
        };

        match send_request() {
            Ok(value) => {
                response.list = Some(value);
                response.successful = true;
                response.message = "List fetched succesfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn update_list(&self, request: Request<List>) -> Result<Response<ListResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let list = request.into_inner();
        let mut response = ListResponse::default();

        let send_request = || -> anyhow::Result<()> {
            let list: QueryableList = list.into();

            diesel::update(lists.filter(id_list.eq(list.id_list.clone())))
                .set((
                    name.eq(list.name.clone()),
                    is_owner.eq(list.is_owner),
                    icon_name.eq(list.icon_name),
                    provider.eq(list.provider),
                ))
                .execute(&mut establish_connection()?)
                .context("Failed to update list.")?;

            Ok(())
        };

        match send_request() {
            Ok(()) => {
                response.list = None;
                response.successful = true;
                response.message = "List updated succesfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }

    async fn delete_list(
        &self,
        request: Request<String>,
    ) -> Result<Response<ListResponse>, Status> {
        tracing::info!("Request received: {request:?}");
        let id = request.into_inner();
        let mut response = ListResponse::default();

        let send_request = || -> anyhow::Result<()> {
            diesel::delete(lists.filter(id_list.eq(id))).execute(&mut establish_connection()?)?;

            Ok(())
        };

        match send_request() {
            Ok(()) => {
                response.list = None;
                response.successful = true;
                response.message = "List removed succesfully.".to_string()
            }
            Err(err) => response.message = err.to_string(),
        }
        Ok(Response::new(response))
    }
}
