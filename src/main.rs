use std::path::Path;

use kasane::Kasane;

fn main() {
    let path = Path::new("data.kasane");
    let kasane = Kasane::init(path).unwrap();

    let write = kasane.write_tx().unwrap();

    write.create_field("neko").unwrap();

    // let fileds = write.show_fileds().unwrap();

    // let _ = write.commit();

    // for ele in fileds {
    //     println!("{}", ele);
    // }
}
