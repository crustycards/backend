use resource_name::*;
use cards_proto::UserProfileImage;
use dashmap::DashMap;

// TODO - Rather than storing profile images locally, let's connect this to an Amazon S3 instance.
pub struct ProfileImageHandler {
    profile_images: DashMap<String, Vec<u8>>,
}

impl ProfileImageHandler {
    pub fn new() -> Self {
        ProfileImageHandler {
            profile_images: DashMap::new(),
        }
    }

    fn empty_profile_image(user_profile_image_name: &UserProfileImageName) -> UserProfileImage {
        UserProfileImage {
            name: user_profile_image_name.clone_str(),
            image_data: Vec::new(),
        }
    }

    pub fn set_profile_image(
        &self,
        user_profile_image_name: &UserProfileImageName,
        image_data: Vec<u8>,
    ) -> UserProfileImage {
        self.profile_images.insert(
            user_profile_image_name.get_object_id().to_hex(),
            image_data.clone(),
        );
        UserProfileImage {
            name: user_profile_image_name.clone_str(),
            image_data,
        }
    }

    pub fn clear_profile_image(
        &self,
        user_profile_image_name: &UserProfileImageName,
    ) -> UserProfileImage {
        self.profile_images
            .remove(&user_profile_image_name.get_object_id().to_hex());
        return Self::empty_profile_image(&user_profile_image_name);
    }

    pub fn get_profile_image(
        &self,
        user_profile_image_name: &UserProfileImageName,
    ) -> UserProfileImage {
        let image_data = match self
            .profile_images
            .get(&user_profile_image_name.get_object_id().to_hex())
        {
            Some(entry) => entry.value().clone(),
            None => return Self::empty_profile_image(&user_profile_image_name),
        };
        UserProfileImage {
            name: user_profile_image_name.clone_str(),
            image_data,
        }
    }
}
