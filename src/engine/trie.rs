use std::collections::HashMap;
use crate::config::Snippet;

/// A Trie node for storing text snippets
#[derive(Debug, Default)]
pub struct TrieNode {
    children: HashMap<char, TrieNode>,
    /// If this node marks the end of a trigger, store the snippet here
    snippet: Option<Snippet>,
}

impl TrieNode {
    fn new() -> Self {
        Self::default()
    }
}

/// A Trie for efficient prefix/suffix matching of triggers
pub struct Trie {
    root: TrieNode,
}

impl Trie {
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Insert a snippet into the trie
    /// We insert the trigger in REVERSE order to support efficient suffix matching
    /// (matching as the user types backward from the cursor)
    pub fn insert(&mut self, snippet: Snippet) {
        let text = snippet.trigger.clone();
        // Since we match what the user *just typed*, we look at the end of the buffer
        // So a structure that supports searching from the end is better.
        // We realize this is a Suffix match on the buffer.
        // We will insert the trigger reversed, so ";email" becomes "l", "i", "a"...
        // Then we can walk the trie with the reversed buffer.
        
        let mut node = &mut self.root;
        for ch in text.chars().rev() {
            node = node.children.entry(ch).or_insert_with(TrieNode::new);
        }
        node.snippet = Some(snippet);
    }

    /// Find a matching snippet for the end of the given text
    /// Returns the matched snippet and the length of the matched trigger
    pub fn find_match(&self, params: &str) -> Option<(&Snippet, usize)> {
        let mut node = &self.root;
        let mut depth = 0;
        let mut best_match = None;
        
        // Walk backwards from the end of the input
        for ch in params.chars().rev() {
            if let Some(next_node) = node.children.get(&ch) {
                node = next_node;
                depth += 1;
                
                // If this node is a terminal node, we found a match!
                // We keep searching to find the *longest* match if there are overlapping triggers
                if let Some(snippet) = &node.snippet {
                    best_match = Some((snippet, depth));
                }
            } else {
                break;
            }
        }
        
        best_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_snippet(trigger: &str) -> Snippet {
        Snippet {
            trigger: trigger.to_string(),
            replace: "content".to_string(),
            label: None,
            propagate_case: false,
            cursor_position: false,
            word_boundary: false,
            regex: false,
            applications: None,
            exclude_applications: None,
            enabled: true,
        }
    }

    #[test]
    fn test_reverse_lookup() {
        let mut trie = Trie::new();
        trie.insert(make_snippet(";test"));
        
        // Exact match
        let (s, len) = trie.find_match("hello ;test").unwrap();
        assert_eq!(s.trigger, ";test");
        assert_eq!(len, 5);
        
        // Match at end of longer string
        let (s, len) = trie.find_match("this is a ;test").unwrap();
        assert_eq!(s.trigger, ";test");
        assert_eq!(len, 5);
        
        // Partial match fail
        assert!(trie.find_match(";tes").is_none());
        
        // No match
        assert!(trie.find_match("nothing here").is_none());
    }
    #[test]
    fn test_longest_match_priority() {
        let mut trie = Trie::new();
        trie.insert(make_snippet("test"));
        trie.insert(make_snippet(";test"));

        // When we type ";test", it contains both "test" (suffix) and ";test" (longer suffix).
        // We want the longest match.
        let (s, len) = trie.find_match("hello ;test").unwrap();
        assert_eq!(s.trigger, ";test");
        assert_eq!(len, 5);
    }
}
