use crate::{
    models::Hash,
    utils::{util, Store},
};
use std::{fs, path::Path};

/**Blob<br>
git中最基本的对象，他储存一份文件的内容，并使用hash作为标识符。
*/
#[derive(Debug, Clone)]
pub struct Blob {
    hash: Hash,
    data: String,
}

impl Blob {
    /// 从源文件新建blob对象，并直接保存到/objects/中
    pub fn new(file: &Path) -> Blob {
        let data = fs::read_to_string(file).expect("无法读取文件");
        let hash = util::calc_hash(&data);
        let blob = Blob { hash, data };
        blob.save();
        blob
    }

    /// 通过hash值加载blob（从/objects/）
    #[allow(dead_code)]
    pub fn load(hash: &String) -> Blob {
        let s = Store::new();
        let data = s.load(hash);
        Blob { hash: hash.clone(), data }
    }

    ///将hash对应的blob还原到file
    pub fn restore(&self, file: &Path) {
        util::write(file, &self.data).unwrap();
    }

    /// 写入文件；优化：文件已存在时不做操作
    pub fn save(&self) {
        let s = Store::new();
        if !s.contains(&self.hash) {
            let hash = s.save(&self.data);
            assert_eq!(hash, self.hash);
        }
    }

    pub fn get_hash(&self) -> String {
        self.hash.clone()
    }
}
