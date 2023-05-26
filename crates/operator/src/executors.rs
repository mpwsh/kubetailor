use crate::prelude::*;


pub fn new<T: Resource + HasSpec + Default>(
    namespace: &str,
    name: &str,
    app: &T,
    build_spec: fn(&T) -> T::Spec,
) -> T {
    let mut resource = T::default();
    resource.meta().name = Some(name.to_owned());
    resource.meta().namespace = Some(namespace.to_owned());
    resource.spec(build_spec(app));
    resource
}


pub async fn deploy<T>(
    client: Client,
    namespace: &str,
    name: &str,
    app: &T,
    build_spec: fn(&T) -> T::Spec,
) -> Result<T, Error>
where
    T: Clone + Debug + Resource<DynamicType = ()> + HasSpec + Default + 'static,
{
    let resource = new(namespace, name, &app.clone(), build_spec);
    let api: Api<T> = Api::namespaced(client.clone(), namespace);
    match api.create(&PostParams::default(), &resource).await {
        Ok(res) => Ok(res),
        Err(Error::Api(e)) if e.code == 409 => {
            // Resource already exists, update it
            update(client, namespace, name, app, build_spec).await
        }
        Err(e) => Err(e),
    }
}

pub async fn update<T>(
    client: Client,
    namespace: &str,
    name: &str,
    app: &T,
    build_spec: fn(&T) -> T::Spec,
) -> Result<T, Error>
where
    T: Clone + Debug + Resource<DynamicType = ()> + HasSpec + Default + 'static,
{
    let mut resource = new(namespace, name, &app.clone(), build_spec);
    let api: Api<T> = Api::namespaced(client, namespace);

    let resource_version = api.get(name).await?.metadata.resource_version;
    resource.metadata.resource_version = resource_version;

    api.replace(name, &PostParams::default(), &resource).await
}

pub async fn delete<T>(client: Client, namespace: &str, name: &str) -> Result<(), Error>
where
    T: Resource,
{
    let api: Api<T> = Api::namespaced(client, namespace);
    api.delete(name, &DeleteParams::default()).await?;
    Ok(())
}

pub async fn exists<T>(client: Client, namespace: &str, name: &str) -> Result<bool, Error>
where
    T: Resource,
{
    let api: Api<T> = Api::namespaced(client, namespace);
    match api.get(name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(e),
    }
}
