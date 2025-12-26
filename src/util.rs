use regex::Regex;

pub mod report_err {
    use tokio::sync::mpsc::Sender;

    pub trait CloneReportErr<T, E> {
        fn report_err(self, tx: &Sender<E>) -> Self;
    }

    impl<T, E> CloneReportErr<T, E> for Result<T, E>
    where
        E: Send + Sync + Clone,
    {
        fn report_err(self, tx: &Sender<E>) -> Self {
            if let Err(ref err_original) = self
                && let Err(err) = tx.try_send(err_original.clone())
            {
                tracing::error!("Failed to report error to the user: {err:?}");
                tracing::error!("Original error: {err:?}");
            }
            self
        }
    }

    pub trait OkReportErr<T, E> {
        fn ok_or_report(self, tx: &Sender<E>) -> Option<T>;
    }

    impl<T, E> OkReportErr<T, E> for Result<T, E>
    where
        E: Send + Sync,
    {
        fn ok_or_report(self, tx: &Sender<E>) -> Option<T> {
            match self {
                Ok(val) => Some(val),
                Err(err) => {
                    if let Err(err) = tx.try_send(err) {
                        tracing::error!("Failed to report error to the user: {err:?}");
                    };
                    None
                }
            }
        }
    }

    pub trait UnwrapReportErr<E> {
        fn unwrap_or_report(self, tx: &Sender<E>);
    }

    impl<E> UnwrapReportErr<E> for Result<(), E>
    where
        E: Send + Sync,
    {
        fn unwrap_or_report(self, tx: &Sender<E>) {
            self.unwrap_or_else(|err| {
                if let Err(err) = tx.try_send(err) {
                    tracing::error!("Failed to report error to the user: {err:?}");
                };
            })
        }
    }
}

pub fn slugify<T: From<&str>>(s: &str) -> T {
    let s = s.to_lowercase();
    let re = Regex::new(r"[^\w\d]+").unwrap();
    T::from(re.replace_all(&s, "-").trim_matches('-'))
}
