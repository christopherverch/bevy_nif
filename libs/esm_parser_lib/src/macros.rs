#[macro_export]
macro_rules! load_subrecords {
    ($reader:expr, $($name:literal => $action:block),* $(,)? ) => {{
        while $reader.has_more_subs() {
            let sub_name = $reader.get_or_read_sub_name()?;

            match &sub_name.data {
                $($name => {
                    $action
                })*
                _ => {
                    return Err(EsmError::UnknownRecName(u32::from_le_bytes(sub_name.data)));

                }
            }
        }
    }};
}
