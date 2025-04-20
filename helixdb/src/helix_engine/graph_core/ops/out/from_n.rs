use crate::helix_engine::{
    graph_core::{ops::tr_val::TraversalVal, traversal_iter::RoTraversalIterator},
    storage_core::{storage_core::HelixGraphStorage, storage_methods::StorageMethods},
    types::GraphError,
};
use heed3::{RoTxn, RwTxn};
use std::sync::Arc;

pub struct FromNIterator<'a, I, T> {
    iter: I,
    storage: Arc<HelixGraphStorage>,
    txn: &'a T,
}

// implementing iterator for OutIterator
impl<'a, I> Iterator for FromNIterator<'a, I, RoTxn<'a>>
where
    I: Iterator<Item = Result<TraversalVal, GraphError>>,
{
    type Item = Result<TraversalVal, GraphError>;

    /// Returns the next outgoing node by decoding the edge id and then getting the edge and node
    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(item) => match item {
                Ok(TraversalVal::Edge(item)) => Some(Ok(TraversalVal::Node(
                    self.storage.get_node(self.txn, &item.from_node).unwrap(),
                ))), // TODO: handle unwrap
                _ => return None,
            },
            None => None,
        }
    }
}

impl<'a, I> Iterator for FromNIterator<'a, I, RwTxn<'a>>
where
    I: Iterator<Item = Result<TraversalVal, GraphError>>,
{
    type Item = Result<TraversalVal, GraphError>;

    /// Returns the next outgoing node by decoding the edge id and then getting the edge and node
    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(item) => match item {
                Ok(TraversalVal::Edge(item)) => Some(Ok(TraversalVal::Node(
                    self.storage.get_node(self.txn, &item.from_node).unwrap(),
                ))), // TODO: handle unwrap
                _ => return None,
            },
            None => None,
        }
    }
}
pub trait FromNAdapter<'a, T>: Iterator<Item = Result<TraversalVal, GraphError>> + Sized {
    fn from_n(
        self,
    ) -> RoTraversalIterator<'a, impl Iterator<Item = Result<TraversalVal, GraphError>>>;
}

impl<'a, I: Iterator<Item = Result<TraversalVal, GraphError>> + 'a> FromNAdapter<'a, RoTxn<'a>>
    for RoTraversalIterator<'a, I>
{
    fn from_n(
        self,
    ) -> RoTraversalIterator<'a, impl Iterator<Item = Result<TraversalVal, GraphError>>> {
        let iter = FromNIterator {
            iter: self.inner,
            storage: Arc::clone(&self.storage),
            txn: self.txn,
        };
        RoTraversalIterator {
            inner: iter,
            storage: self.storage,
            txn: self.txn,
        }
    }
}

// impl<'a, I: Iterator<Item = Result<TraversalVal, GraphError>> + 'a> OutNAdapter<'a, RwTxn<'a>>
//     for RwTraversalIterator<'a, I>
// {
//     fn out_n(
//         self,
//         db: Arc<HelixGraphStorage>,
//         txn: &'a RwTxn<'a>,
//     ) -> OutVIterator<'a, Self, RwTxn<'a>> {
//         OutVIterator {
//             iter: self,
//             storage: db,
//             txn,
//         }
//     }
// }
