#[macro_export]
macro_rules! location {
    () => {
        format!("{}:{}:{}", module_path!(), file!(), line!())
    };
}
