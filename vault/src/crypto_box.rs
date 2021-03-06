// Copyright 2020 IOTA Stiftung
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on
// an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and limitations under the License.

use std::{
    convert::TryFrom,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use serde::{Deserialize, Serialize};

/// A provider interface between the vault and a crypto box. See libsodium's [secretbox](https://libsodium.gitbook.io/doc/secret-key_cryptography/secretbox) for an example.
pub trait BoxProvider: Sized {
    /// function for the key length of the crypto box
    fn box_key_len() -> usize;
    /// gets the crypto box's overhead
    fn box_overhead() -> usize;

    /// seals some data into the crypto box using the `key` and the `ad`
    fn box_seal(key: &Key<Self>, ad: &[u8], data: &[u8]) -> crate::Result<Vec<u8>>;

    /// opens a crypto box to get data using the `key` and the `ad`.
    fn box_open(key: &Key<Self>, ad: &[u8], data: &[u8]) -> crate::Result<Vec<u8>>;

    /// fills a buffer `buf` with secure random bytes.
    fn random_buf(buf: &mut [u8]) -> crate::Result<()>;

    /// creates a vector with secure random bytes based off of an inputted length `len`.
    fn random_vec(len: usize) -> crate::Result<Vec<u8>> {
        let mut buf = vec![0; len];
        Self::random_buf(&mut buf)?;
        Ok(buf)
    }
}

/// A key to the crypto box.  Key is stored on the heap which makes it easier to erase.
#[derive(Serialize, Deserialize)]
pub struct Key<T: BoxProvider> {
    /// the raw bytes that make up the key
    pub key: Vec<u8>,
    /// callback function invoked on drop. Used top drop the data out of memory or pass it to a file.
    #[serde(skip_serializing, skip_deserializing)]
    drop_fn: Option<&'static fn(&mut [u8])>,
    /// associated Provider data
    _box_provider: PhantomData<T>,
}

impl<T: BoxProvider> Key<T> {
    /// generate a random key using secure random bytes
    pub fn random() -> crate::Result<Self> {
        Ok(Self {
            key: T::random_vec(T::box_key_len())?,
            drop_fn: None,
            _box_provider: PhantomData,
        })
    }

    /// attempts to load a key from inputted data
    pub fn load(key: Vec<u8>) -> crate::Result<Self> {
        match key {
            key if key.len() != T::box_key_len() => Err(crate::Error::InterfaceError),
            key => Ok(Self {
                key,
                drop_fn: None,
                _box_provider: PhantomData,
            }),
        }
    }

    /// set up the drop hook function which will be called if the instance gets dropped
    pub fn on_drop(&mut self, hook: &'static fn(&mut [u8])) {
        self.drop_fn = Some(hook)
    }

    /// get the key's bytes
    pub fn bytes(&self) -> &[u8] {
        &self.key
    }
}

impl<T: BoxProvider> Clone for Key<T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            drop_fn: self.drop_fn,
            _box_provider: PhantomData,
        }
    }
}

impl<T: BoxProvider> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self._box_provider == other._box_provider
    }
}

impl<T: BoxProvider> Eq for Key<T> {}

impl<T: BoxProvider> Hash for Key<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self._box_provider.hash(state);
    }
}

/// call the drop hook on dropping the key.
impl<T: BoxProvider> Drop for Key<T> {
    fn drop(&mut self) {
        if let Some(hook) = self.drop_fn {
            hook(&mut self.key);
        }
    }
}

use std::fmt::Debug;

impl<T: BoxProvider> Debug for Key<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Key").field("key data", &self.key).finish()
    }
}

/// trait for encryptable data
pub trait Encrypt<T: From<Vec<u8>>>: AsRef<[u8]> {
    /// encrypts a raw data and creates a type T from the ciphertext
    fn encrypt<B: BoxProvider>(&self, key: &Key<B>, ad: &[u8]) -> crate::Result<T> {
        let sealed = B::box_seal(key, ad, self.as_ref())?;
        Ok(T::from(sealed))
    }
}

/// Trait for decryptable data
pub trait Decrypt<E, T: TryFrom<Vec<u8>, Error = E>>: AsRef<[u8]> {
    /// decrypts raw data and creates a new type T from the plaintext
    fn decrypt<B: BoxProvider>(&self, key: &Key<B>, ad: &[u8]) -> crate::Result<T> {
        let opened = B::box_open(key, ad, self.as_ref())?;
        Ok(T::try_from(opened).map_err(|_| crate::Error::DatabaseError(String::from("Invalid Entry")))?)
    }
}
