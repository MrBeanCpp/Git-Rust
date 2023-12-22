use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{store, utils::util};

use super::{index::Index, object::Hash};
/*Tree
* Tree是一个版本中所有文件的集合。从根目录还是，每个目录是一个Tree，每个文件是一个Blob。Tree之间互相嵌套表示文件的层级关系。
* 每一个Tree对象也是对应到git储存仓库的一个文件，其内容是一个或多个TreeEntry。
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TreeEntry {
    pub filemode: (String, String), // (type, mode), type: blob or tree; mode: 100644 or 04000
    pub object_hash: Hash,          // blob hash or tree hash
    pub name: String,               // file name
}

/// 相对路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    #[serde(skip)]
    pub hash: Hash,
    pub entries: Vec<TreeEntry>,
}

/** 将文件列表保存为Tree Object，并返回最上层的Tree */
fn store_path_to_tree(path_entries: &Vec<PathBuf>, current_root: PathBuf) -> Tree {
    let get_blob_entry = |path: &PathBuf| {
        let file_path = util::get_working_dir().unwrap().join(path);
        let blob = super::blob::Blob::new(&file_path.clone());
        let mode = util::get_file_mode(&path);
        let filename = path.file_name().unwrap().to_str().unwrap().to_string();
        let entry = TreeEntry {
            filemode: (String::from("blob"), mode),
            object_hash: blob.get_hash(),
            name: filename,
        };
        entry
    };
    let mut tree = Tree { hash: "".to_string(), entries: Vec::new() };
    let mut processed_path: HashMap<String, bool> = HashMap::new();
    for path in path_entries.iter() {
        // 判断是不是直接在根目录下
        let in_path = path.parent().unwrap() == current_root;
        // 一定是文件，不会是目录
        if in_path {
            let entry = get_blob_entry(path);
            tree.entries.push(entry);
        } else {
            if path.components().count() == 1 {
                continue;
            }
            // 拿到下一级别目录
            let process_path = path.components().nth(0).unwrap().as_os_str().to_str().unwrap();
            if processed_path.insert(process_path.to_string(), true).is_some() {
                continue;
            }

            let sub_tree = store_path_to_tree(path_entries, process_path.into());
            let mode = util::get_file_mode(&util::get_working_dir().unwrap().join(process_path));
            tree.entries.push(TreeEntry {
                filemode: (String::from("tree"), mode),
                object_hash: sub_tree.get_hash(),
                name: process_path.to_string(),
            });
        }
    }
    tree.save();
    tree
}

impl Tree {
    pub fn get_hash(&self) -> String {
        self.hash.clone()
    }

    pub fn new(index: &Index) -> Tree {
        let file_entries: Vec<PathBuf> = index
            .get_tracked_files()
            .iter_mut()
            .map(|file| util::to_workdir_relative_path(file))
            .collect();

        store_path_to_tree(&file_entries, "".into())
    }

    pub fn load(hash: &String) -> Tree {
        let s = store::Store::new();
        let tree_data = s.load(hash);
        let mut tree: Tree = serde_json::from_str(&tree_data).unwrap();
        tree.hash = hash.clone();
        tree
    }

    pub fn save(&mut self) -> String {
        let s = store::Store::new();
        let tree_data = serde_json::to_string_pretty(&self).unwrap();
        let hash = s.save(&tree_data);
        self.hash = hash.clone();
        hash
    }

    /**递归获取Tree对应的所有文件 */
    pub fn get_recursive_file_entries(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in self.entries.iter() {
            if entry.filemode.0 == "blob" {
                files.push(PathBuf::from(entry.name.clone()));
            } else {
                let sub_tree = Tree::load(&entry.object_hash);
                let sub_files = sub_tree.get_recursive_file_entries();

                files.append(
                    sub_files
                        .iter()
                        .map(|file| PathBuf::from(entry.name.clone()).join(file))
                        .collect::<Vec<PathBuf>>()
                        .as_mut(),
                );
            }
        }
        files
    }

    ///注：相对路径
    pub fn get_recursive_blobs(&self) -> Vec<(PathBuf, Hash)> {
        let mut blob_hashs = Vec::new();
        for entry in self.entries.iter() {
            if entry.filemode.0 == "blob" {
                blob_hashs.push((PathBuf::from(entry.name.clone()), entry.object_hash.clone()));
            } else {
                let sub_tree = Tree::load(&entry.object_hash);
                let sub_blobs = sub_tree.get_recursive_blobs();

                blob_hashs.append(
                    sub_blobs
                        .iter()
                        .map(|(path, blob_hash)| (PathBuf::from(entry.name.clone()).join(path), blob_hash.clone()))
                        .collect::<Vec<(PathBuf, Hash)>>()
                        .as_mut(),
                );
            }
        }
        blob_hashs
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::models::blob::Blob;
    use crate::models::index::FileMetaData;
    use crate::utils::util;
    #[test]
    fn test_new() {
        util::setup_test_with_clean_mit();
        let mut index = super::Index::new();
        for test_file in vec!["b.txt", "mit_src/a.txt"] {
            let test_file = PathBuf::from(test_file);
            util::ensure_test_file(&test_file, None);
            index.add(test_file.clone(), FileMetaData::new(&Blob::new(&test_file), &test_file));
            index.add(test_file.clone(), FileMetaData::new(&Blob::new(&test_file), &test_file));
        }

        let tree = super::Tree::new(&index);
        assert!(tree.entries.len() == 2);
        assert!(tree.hash.len() != 0);
    }

    #[test]
    fn test_load() {
        util::setup_test_with_clean_mit();
        let mut index = super::Index::new();
        let test_files = vec!["b.txt", "mit_src/a.txt"];
        for test_file in test_files.clone() {
            let test_file = PathBuf::from(test_file);
            util::ensure_test_file(&test_file, None);
            index.add(test_file.clone(), FileMetaData::new(&Blob::new(&test_file), &test_file));
        }

        let tree = super::Tree::new(&index);
        let tree_hash = tree.get_hash();

        let loaded_tree = super::Tree::load(&tree_hash);
        assert!(loaded_tree.entries.len() == tree.entries.len());
        assert!(tree.entries[0].name == loaded_tree.entries[0].name);
        assert!(tree.entries[1].name == loaded_tree.entries[1].name);
    }

    #[test]
    fn test_get_recursive_file_entries() {
        util::setup_test_with_clean_mit();
        let mut index = super::Index::new();
        let test_files = vec!["b.txt", "mit_src/a.txt"];
        for test_file in test_files.clone() {
            let test_file = PathBuf::from(test_file);
            util::ensure_test_file(&test_file, None);
            index.add(test_file.clone(), FileMetaData::new(&Blob::new(&test_file), &test_file));
        }

        let tree = super::Tree::new(&index);
        let tree_hash = tree.get_hash();

        let loaded_tree = super::Tree::load(&tree_hash);
        let files = loaded_tree.get_recursive_file_entries();
        assert!(files.len() == test_files.len());
        assert!(files[0].to_str().unwrap() == test_files[0]);
        assert!(files[1].to_str().unwrap() == test_files[1]);
    }

    #[test]
    fn test_get_recursive_blobs() {
        util::setup_test_with_clean_mit();
        let mut index = super::Index::new();
        let test_files = vec!["b.txt", "mit_src/a.txt"];
        let mut test_blobs = vec![];
        for test_file in test_files.clone() {
            let test_file = PathBuf::from(test_file);
            util::ensure_test_file(&test_file, None);
            let blob = Blob::new(&test_file);
            test_blobs.push(blob.clone());
            index.add(test_file.clone(), FileMetaData::new(&Blob::new(&test_file), &test_file));
        }

        let tree = super::Tree::new(&index);
        let tree_hash = tree.get_hash();

        let loaded_tree = super::Tree::load(&tree_hash);
        let blobs = loaded_tree.get_recursive_blobs();
        assert!(blobs.len() == test_files.len());
        assert!(blobs.contains(&(PathBuf::from(test_files[0]), test_blobs[0].get_hash())));
        assert!(blobs.contains(&(PathBuf::from(test_files[1]), test_blobs[1].get_hash())));
    }
}
