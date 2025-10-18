use std::borrow::Cow;
use std::sync::Arc;
use parking_lot::Mutex;

/// 字符串对象池（减少堆分配）
pub struct StringPool {
    pool: Arc<Mutex<Vec<String>>>,
    max_size: usize,
}

impl StringPool {
    /// 创建新的字符串池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            max_size,
        }
    }

    /// 从池中获取字符串，如果池为空则创建新的
    pub fn acquire(&self) -> PooledString {
        let mut pool = self.pool.lock();
        let string = pool.pop().unwrap_or_else(String::new);
        PooledString {
            string: Some(string),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }

    /// 获取当前池大小
    pub fn size(&self) -> usize {
        self.pool.lock().len()
    }
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new(1000) // 默认池大小1000
    }
}

/// 池化的字符串（自动归还）
pub struct PooledString {
    string: Option<String>,
    pool: Arc<Mutex<Vec<String>>>,
    max_size: usize,
}

impl PooledString {
    /// 获取内部字符串的可变引用
    pub fn as_mut(&mut self) -> &mut String {
        self.string.as_mut().unwrap()
    }

    /// 获取内部字符串的引用
    pub fn as_str(&self) -> &str {
        self.string.as_ref().unwrap().as_str()
    }

    /// 设置内容
    pub fn set(&mut self, content: &str) {
        if let Some(s) = &mut self.string {
            s.clear();
            s.push_str(content);
        }
    }
}

impl Drop for PooledString {
    fn drop(&mut self) {
        if let Some(mut string) = self.string.take() {
            string.clear();
            let mut pool = self.pool.lock();
            // 只有池未满时才归还
            if pool.len() < self.max_size {
                pool.push(string);
            }
        }
    }
}

impl std::fmt::Display for PooledString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// HashMap值对象池（减少Vec分配）
pub struct VecPool<T> {
    pool: Arc<Mutex<Vec<Vec<T>>>>,
    max_size: usize,
}

impl<T> VecPool<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(Vec::with_capacity(max_size))),
            max_size,
        }
    }

    pub fn acquire(&self) -> PooledVec<T> {
        let mut pool = self.pool.lock();
        let vec = pool.pop().unwrap_or_else(Vec::new);
        PooledVec {
            vec: Some(vec),
            pool: Arc::clone(&self.pool),
            max_size: self.max_size,
        }
    }
}

impl<T> Default for VecPool<T> {
    fn default() -> Self {
        Self::new(500)
    }
}

pub struct PooledVec<T> {
    vec: Option<Vec<T>>,
    pool: Arc<Mutex<Vec<Vec<T>>>>,
    max_size: usize,
}

impl<T> PooledVec<T> {
    pub fn as_mut(&mut self) -> &mut Vec<T> {
        self.vec.as_mut().unwrap()
    }

    pub fn as_slice(&self) -> &[T] {
        self.vec.as_ref().unwrap().as_slice()
    }
}

impl<T> Drop for PooledVec<T> {
    fn drop(&mut self) {
        if let Some(mut vec) = self.vec.take() {
            vec.clear();
            let mut pool = self.pool.lock();
            if pool.len() < self.max_size {
                pool.push(vec);
            }
        }
    }
}

impl<T> std::ops::Deref for PooledVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        self.vec.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.vec.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_pool() {
        let pool = StringPool::new(10);
        
        {
            let mut s1 = pool.acquire();
            s1.set("test1");
            assert_eq!(s1.as_str(), "test1");
        } // s1 归还到池
        
        assert_eq!(pool.size(), 1);
        
        {
            let mut s2 = pool.acquire(); // 复用 s1
            s2.set("test2");
            assert_eq!(s2.as_str(), "test2");
        }
    }

    #[test]
    fn test_vec_pool() {
        let pool: VecPool<i32> = VecPool::new(10);
        
        {
            let mut v1 = pool.acquire();
            v1.push(1);
            v1.push(2);
            assert_eq!(v1.len(), 2);
        }
        
        {
            let v2 = pool.acquire();
            assert_eq!(v2.len(), 0); // 已清空
        }
    }
}

