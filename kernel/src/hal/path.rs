extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Represents an absolute Unix path
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    raw: String,
}

impl Path {
    /// Create a new absolute UnixPath from a String
    /// Returns None if the path is not absolute (doesn't start with '/')
    pub fn new(path: String) -> Option<Self> {
        if path.starts_with('/') {
            Some(Self { raw: path })
        } else {
            None
        }
    }

    /// Create a new absolute UnixPath from a string slice
    /// Returns None if the path is not absolute (doesn't start with '/')
    pub fn from_str(path: &str) -> Option<Self> {
        Self::new(String::from(path))
    }

    /// Create a new absolute UnixPath without checking if it's absolute
    ///
    /// # Safety
    /// The caller must ensure the path starts with '/'
    pub unsafe fn new_unchecked(path: String) -> Self {
        Self { raw: path }
    }

    /// Returns the raw path string as a string slice
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Consumes self and returns the inner String
    pub fn into_string(self) -> String {
        self.raw
    }

    /// Iterator over path components, skipping empty components and the root
    pub fn components(&self) -> Components {
        Components {
            path: self.raw.clone(),
            front: 1, // Start after the leading '/'
            back: self.raw.len(),
        }
    }

    /// Get the file name (last component) if present
    pub fn file_name(&self) -> Option<String> {
        self.components().next_back()
    }

    /// Get the parent path (everything except the last component)
    /// Returns None if this is the root path "/"
    pub fn parent(&self) -> Option<Path> {
        if self.raw == "/" {
            return None;
        }

        let trimmed = self.raw.trim_end_matches('/');
        if let Some(pos) = trimmed.rfind('/') {
            if pos == 0 {
                // Parent is root
                Some(Path {
                    raw: String::from("/"),
                })
            } else {
                Some(Path {
                    raw: String::from(&self.raw[..pos]),
                })
            }
        } else {
            // Should not happen for absolute paths, but return root as fallback
            Some(Path {
                raw: String::from("/"),
            })
        }
    }

    /// Get the extension of the file name, if any
    pub fn extension(&self) -> Option<String> {
        let name = self.file_name()?;
        let pos = name.rfind('.')?;

        // Don't treat dotfiles without extension as having an extension
        if pos == 0 || name[..pos].ends_with('/') {
            return None;
        }

        Some(String::from(&name[pos + 1..]))
    }

    /// Normalize the path by removing '.' and '..' components
    pub fn normalize(&self) -> Path {
        let mut stack: Vec<String> = Vec::new();

        for component in self.components() {
            match component.as_str() {
                "." => continue,
                ".." => {
                    stack.pop();
                }
                _ => {
                    stack.push(component);
                }
            }
        }

        let mut result = String::from("/");

        for (i, component) in stack.iter().enumerate() {
            if i > 0 {
                result.push('/');
            }
            result.push_str(component);
        }

        Path { raw: result }
    }

    /// Join this path with another path component
    /// If other starts with '/', it replaces the entire path
    pub fn join(&self, other: &str) -> Path {
        if other.starts_with('/') {
            // Other is absolute, replace entirely
            return Path {
                raw: String::from(other),
            };
        }

        let mut result = self.raw.clone();

        if !result.ends_with('/') {
            result.push('/');
        }

        result.push_str(other);

        Path { raw: result }
    }

    /// Returns true (always, for compatibility)
    pub fn is_absolute(&self) -> bool {
        true
    }

    /// Returns false (always, for compatibility)
    pub fn is_relative(&self) -> bool {
        false
    }
}

/// Iterator over path components
#[derive(Debug, Clone)]
pub struct Components {
    path: String,
    front: usize,
    back: usize,
}

impl Iterator for Components {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let remaining = &self.path[self.front..self.back];

            if let Some(pos) = remaining.find('/') {
                let component = &remaining[..pos];
                self.front += pos + 1;

                // Skip empty components
                if !component.is_empty() {
                    return Some(String::from(component));
                }
            } else {
                // Last component
                self.front = self.back;
                if !remaining.is_empty() {
                    return Some(String::from(remaining));
                }
                return None;
            }
        }

        None
    }
}

impl DoubleEndedIterator for Components {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let remaining = &self.path[self.front..self.back];

            if let Some(pos) = remaining.rfind('/') {
                let component = &remaining[pos + 1..];
                self.back = self.front + pos;

                // Skip empty components
                if !component.is_empty() {
                    return Some(String::from(component));
                }
            } else {
                // First component (or only component)
                let component = remaining;
                self.back = self.front;

                if !component.is_empty() {
                    return Some(String::from(component));
                }
                return None;
            }
        }

        None
    }
}

impl TryFrom<String> for Path {
    type Error = ();

    fn try_from(path: String) -> Result<Self, Self::Error> {
        Self::new(path).ok_or(())
    }
}

impl TryFrom<&str> for Path {
    type Error = ();

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        Self::from_str(path).ok_or(())
    }
}
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_absolute_path_only() {
//         let path = UnixPath::from_str("/usr/local/bin");
//         assert!(path.is_some());
//
//         let path = UnixPath::from_str("usr/local/bin");
//         assert!(path.is_none());
//     }
//
//     #[test]
//     fn test_is_absolute() {
//         let path = UnixPath::from_str("/usr/local/bin").unwrap();
//         assert!(path.is_absolute());
//         assert!(!path.is_relative());
//     }
//
//     #[test]
//     fn test_components() {
//         let path = UnixPath::from_str("/usr/local/bin").unwrap();
//         let components: Vec<String> = path.components().collect();
//         assert_eq!(components, vec!["usr", "local", "bin"]);
//     }
//
//     #[test]
//     fn test_file_name() {
//         let path = UnixPath::from_str("/usr/local/bin/rustc").unwrap();
//         assert_eq!(path.file_name(), Some(String::from("rustc")));
//     }
//
//     #[test]
//     fn test_parent() {
//         let path = UnixPath::from_str("/usr/local/bin").unwrap();
//         let parent = path.parent().unwrap();
//         assert_eq!(parent.as_str(), "/usr/local");
//
//         let root = UnixPath::from_str("/").unwrap();
//         assert!(root.parent().is_none());
//     }
//
//     #[test]
//     fn test_extension() {
//         let path = UnixPath::from_str("/path/to/file.txt").unwrap();
//         assert_eq!(path.extension(), Some(String::from("txt")));
//
//         let path = UnixPath::from_str("/path/to/.hidden").unwrap();
//         assert_eq!(path.extension(), None);
//     }
//
//     #[test]
//     fn test_normalize() {
//         let path = UnixPath::from_str("/usr/./local/../bin").unwrap();
//         let normalized = path.normalize();
//         assert_eq!(normalized.as_str(), "/usr/bin");
//
//         let path = UnixPath::from_str("/usr/local/../../bin").unwrap();
//         let normalized = path.normalize();
//         assert_eq!(normalized.as_str(), "/bin");
//     }
//
//     #[test]
//     fn test_join() {
//         let path = UnixPath::from_str("/usr/local").unwrap();
//         let joined = path.join("bin");
//         assert_eq!(joined.as_str(), "/usr/local/bin");
//
//         let path = UnixPath::from_str("/usr/local").unwrap();
//         let joined = path.join("/etc");
//         assert_eq!(joined.as_str(), "/etc");
//     }
// }
