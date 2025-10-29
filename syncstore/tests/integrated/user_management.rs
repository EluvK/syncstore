use crate::mock::*;

#[test]
fn user_create_validate() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();

    // create a new user
    store.create_user("new_user", "password123")?;

    // validate the new user
    let validated_id = store.validate_user("new_user", "password123")?;
    assert!(
        validated_id.is_some(),
        "User should be created and validated successfully"
    );

    let non_existent_user = store.validate_user("non_existent_user", "wrong_password")?;
    assert!(non_existent_user.is_none());

    let wrong_password = store.validate_user("new_user", "wrong_password")?;
    assert!(wrong_password.is_none());

    Ok(())
}
