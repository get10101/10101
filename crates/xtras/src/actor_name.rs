use xtra::Actor;

pub trait ActorName {
    fn name() -> String;
}

impl<T> ActorName for T
where
    T: Actor,
{
    /// Devise the name of an actor from its type on a best-effort
    /// basis.
    fn name() -> String {
        std::any::type_name::<T>().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_name_from_type() {
        let name = Dummy::name();

        assert_eq!(name, "xtras::actor_name::tests::Dummy")
    }

    struct Dummy;

    #[async_trait::async_trait]
    impl Actor for Dummy {
        type Stop = ();

        async fn stopped(self) -> Self::Stop {}
    }
}
