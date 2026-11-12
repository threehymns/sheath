use super::helpers::*;
use crate::drives::{Drive, Drives};
use std::path::PathBuf;

fn mounted(name: &str, path: PathBuf) -> Drive {
    Drive {
        id: format!("volume:{name}"),
        name: name.to_owned(),
        device: format!("/dev/{name}"),
        mountpoint: Some(path),
    }
}

fn unmounted(name: &str) -> Drive {
    Drive {
        id: format!("volume:{name}"),
        name: name.to_owned(),
        device: format!("/dev/{name}"),
        mountpoint: None,
    }
}

#[test]
fn the_sidebar_lists_mounted_and_unmounted_drives() {
    let dir = fixture();
    let mut app = app(&dir);
    app.drives = Drives::fixed(vec![
        mounted("mounted", dir.path().join("alpha")),
        unmounted("stick"),
    ]);
    let mut rt = runtime(app);
    rt.frame(0.0);

    let (row, _) = rt
        .presented("drive-volume:mounted")
        .expect("the mounted drive is on screen");
    let (button, _) = rt
        .presented("drive-unmount-volume:mounted")
        .expect("the mounted drive has an unmount button");
    assert!(
        button[0] >= row[0] && button[0] < row[0] + 40.0,
        "the unmount button sits on the left of the row: {button:?} in {row:?}"
    );
    assert!(
        rt.find("drive-volume:stick").is_some(),
        "the unmounted drive is listed too"
    );
    assert!(
        rt.find("drive-unmount-volume:stick").is_none(),
        "only a mounted drive offers unmount"
    );
}

#[test]
fn clicking_an_unmounted_drive_mounts_it() {
    let dir = fixture();
    let mut app = app(&dir);
    app.drives = Drives::fixed(vec![unmounted("stick")]);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "drive-volume:stick");
    rt.frame(0.1);

    assert!(
        rt.state
            .drives
            .find("volume:stick")
            .is_some_and(Drive::is_mounted),
        "the press mounted the drive"
    );
    assert!(
        rt.find("drive-unmount-volume:stick").is_some(),
        "the row offers unmount once it is mounted"
    );
}

#[test]
fn the_unmount_button_unmounts_without_opening_the_drive() {
    let dir = fixture();
    let mut app = app(&dir);
    app.drives = Drives::fixed(vec![mounted("mounted", dir.path().join("alpha"))]);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "drive-unmount-volume:mounted");
    rt.frame(0.1);

    assert!(
        rt.state
            .drives
            .find("volume:mounted")
            .is_some_and(|drive| !drive.is_mounted()),
        "the button unmounted the drive"
    );
    assert!(
        rt.find("drive-volume:mounted").is_some(),
        "the drive keeps its row while it is unplugged"
    );
    assert!(
        rt.find("drive-unmount-volume:mounted").is_none(),
        "an unmounted drive no longer offers unmount"
    );
}
