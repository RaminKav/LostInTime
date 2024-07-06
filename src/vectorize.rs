pub mod vectorize {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::iter::FromIterator;

    pub fn serialize<'a, T, K, V, S>(target: T, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: IntoIterator<Item = (&'a K, &'a V)>,
        K: Serialize + 'a,
        V: Serialize + 'a,
    {
        let container: Vec<_> = target.into_iter().collect();
        serde::Serialize::serialize(&container, ser)
    }

    pub fn deserialize<'de, T, K, V, D>(des: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromIterator<(K, V)>,
        K: Deserialize<'de>,
        V: Deserialize<'de>,
    {
        let container: Vec<_> = serde::Deserialize::deserialize(des)?;
        Ok(T::from_iter(container.into_iter()))
    }
}
pub mod vectorize_inner {
    use bevy::utils::HashMap;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::iter::FromIterator;

    pub fn serialize<K, V, S>(target: &Vec<HashMap<K, V>>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Serialize,
        V: Serialize,
    {
        let serialized: Vec<_> = target
            .into_iter()
            .map(|hashmap| hashmap.into_iter().collect::<Vec<_>>())
            .collect();

        serde::Serialize::serialize(&serialized, ser)
    }

    pub fn deserialize<'de, K, V, D>(des: D) -> Result<Vec<HashMap<K, V>>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + Eq + std::hash::Hash,
        V: Deserialize<'de>,
    {
        let deserialized: Vec<Vec<(K, V)>> = serde::Deserialize::deserialize(des)?;

        let result: Vec<HashMap<K, V>> = deserialized
            .into_iter()
            .map(|vec| HashMap::from_iter(vec))
            .collect();

        Ok(result)
    }
}
