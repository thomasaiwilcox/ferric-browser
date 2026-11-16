use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, marker::PhantomData, ops::Deref};

/// Bounds sequence allocation while decoding, including rejected payloads.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub(crate) struct List<T, const N: usize>(Vec<T>);

impl<T, const N: usize> Default for List<T, N> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T, const N: usize> List<T, N> {
    pub(super) fn from_vec(values: Vec<T>) -> Option<Self> {
        (values.len() <= N).then_some(Self(values))
    }

    pub(super) fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.0.retain(keep);
    }

    pub(super) fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.0.iter_mut()
    }
}

impl<T, const N: usize> Deref for List<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for List<T, N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<T, const N: usize>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> de::Visitor<'de> for Visitor<T, N> {
            type Value = List<T, N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "at most {N} entries")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(N));
                while values.len() < N {
                    let Some(value) = sequence.next_element()? else {
                        return Ok(List(values));
                    };
                    values.push(value);
                }
                if sequence.next_element::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("annotation sequence limit exceeded"));
                }
                Ok(List(values))
            }
        }
        deserializer.deserialize_seq(Visitor::<T, N>(PhantomData))
    }
}

/// JS transports every u64 as a canonical decimal string, including zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Revision(pub u64);

impl Serialize for Revision {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Revision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = Revision;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a canonical decimal u64 string")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Revision, E> {
                if value.is_empty()
                    || value.len() > 20
                    || (value.len() > 1 && value.starts_with('0'))
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(E::custom("invalid annotation revision"));
                }
                value
                    .parse()
                    .map(Revision)
                    .map_err(|_| E::custom("annotation revision overflow"))
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn sequence_visitors_do_not_construct_entries_past_the_limit() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        struct Counted;
        impl<'de> Deserialize<'de> for Counted {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                COUNT.fetch_add(1, Ordering::Relaxed);
                u32::deserialize(d)?;
                Ok(Self)
            }
        }
        for (raw, valid, count) in [
            ("[1,2]", true, 2),
            ("[1,2,3]", true, 3),
            ("[1,2,3,4]", false, 3),
        ] {
            COUNT.store(0, Ordering::Relaxed);
            assert_eq!(serde_json::from_str::<List<Counted, 3>>(raw).is_ok(), valid);
            assert_eq!(COUNT.load(Ordering::Relaxed), count);
        }
        COUNT.store(0, Ordering::Relaxed);
        assert!(serde_json::from_str::<List<List<Counted, 2>, 2>>("[[1,2],[3,4,5]]").is_err());
        assert_eq!(COUNT.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn revision_strings_are_canonical_across_full_u64_range() {
        for value in [0, 1, 9_007_199_254_740_993, u64::MAX] {
            let encoded = serde_json::to_string(&Revision(value)).unwrap();
            assert_eq!(encoded, format!("\"{value}\""));
            assert_eq!(serde_json::from_str::<Revision>(&encoded).unwrap().0, value);
        }
    }
}
