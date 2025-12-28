use crate::{
    drivers::fs::ext2::structs::Ext2Fs,
    hal::{fs::HalFsIOErr, path::Path},
};

impl Ext2Fs {
    pub async fn delete_file(&mut self, path: Path) -> Result<(), HalFsIOErr> {
        let (directory_inode, file_inode) = self.walk_path(&path).await?;
        self.find_entry_by_name_and_delete(
            &path.file_name().ok_or(HalFsIOErr::BadPath)?,
            &directory_inode.inode,
        )
        .await?;

        let Some(mut file_inode) = file_inode else {
            return Err(HalFsIOErr::NoSuchFileOrDirectory);
        };

        file_inode.inode.i_links_count -= 1;

        if file_inode.inode.i_links_count == 0 {
            todo!("free inode")
        }

        Ok(())
    }
}
