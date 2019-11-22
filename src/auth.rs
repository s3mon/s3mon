use futures::future::Future;
use rusoto_credential::{
    AwsCredentials, CredentialsError, EnvironmentProvider, ProvideAwsCredentials, StaticProvider,
};

pub struct Auth {
    access_key: String,
    secret_key: String,
}

impl Auth {
    pub const fn new(access_key: String, secret_key: String) -> Self {
        Self {
            access_key,
            secret_key,
        }
    }
}

impl ProvideAwsCredentials for Auth {
    type Future = Box<dyn Future<Item = AwsCredentials, Error = CredentialsError> + Send>;

    fn credentials(&self) -> Self::Future {
        let access_key = self.access_key.clone();
        let secret_key = self.secret_key.clone();
        let future = EnvironmentProvider::default()
            .credentials()
            .or_else(|_| -> Self::Future {
                Box::new(StaticProvider::new_minimal(access_key, secret_key).credentials())
            });
        Box::new(future)
    }
}
