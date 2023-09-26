use rust_di::ServiceCollection;

struct Test1 {
    name: String,
}

#[test]
fn demo() {
    let mut services = ServiceCollection::new();

    services.add::<Test1>().with_factory(|_| Test1 {
        name: String::from("Lila"),
    });

    let mut services = services.build();

    let test1 = services.resolve::<Test1>().unwrap();

    assert_eq!(test1.name, String::from("Lila"));
}
