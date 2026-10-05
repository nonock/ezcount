use super::*;

#[test]
fn a_file_name_stays_in_its_folder() {
    for name in ["Trip.csv", "Summer trip (2).pdf", "report", ".hidden"] {
        assert_eq!(plain_file_name(name).unwrap(), name, "{name}");
    }
}

#[test]
fn a_file_name_leading_elsewhere_is_refused() {
    for name in [
        "",
        ".",
        "..",
        "../Trip.csv",
        "folder/Trip.csv",
        "/etc/passwd",
        "Trip.csv/",
    ] {
        assert!(plain_file_name(name).is_err(), "{name}");
    }
    #[cfg(windows)]
    for name in [
        "..\\Trip.csv",
        "folder\\Trip.csv",
        "C:\\Trip.csv",
        "C:Trip.csv",
    ] {
        assert!(plain_file_name(name).is_err(), "{name}");
    }
}

#[test]
fn the_device_says_what_it_can_do() {
    let features = native_features();
    // A computer saves files; a phone shares them and scans codes.
    assert_eq!(features.save, cfg!(desktop));
    assert_eq!(features.scan, cfg!(mobile));
    assert_eq!(features.share, cfg!(target_os = "android"));
}
