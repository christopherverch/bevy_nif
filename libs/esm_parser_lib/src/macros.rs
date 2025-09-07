#[macro_export]
macro_rules! load_subrecords {
    ($reader:expr, $($name:literal => $action:block),* $(,)? ) => {{
        while $reader.has_more_subs() {
            let sub_name = $reader.get_sub_name()?;

            match &sub_name.data {
                $($name => {
                    $action
                })*
                _ => {
                    $reader.skip_sub_data()?;
                }
            }
        }
    }};
}
