//@ wasmtime-flags = '-Wcomponent-model-async'

include!(env!("BINDINGS"));

use crate::my::test::i::*;
use wit_bindgen::StreamResult;

struct Component;

export!(Component);

impl Guest for Component {
    async fn run() {
        let (mut tx, rx) = wit_stream::new();
        let test = async {
            // write one item
            let (result, ret) = tx.write(vec![0]).await;
            assert_eq!(result, StreamResult::Complete(1));
            assert_eq!(ret.remaining(), 0);

            // write two items
            let (result, ret) = tx.write(vec![1, 2]).await;
            assert_eq!(result, StreamResult::Complete(2));
            assert_eq!(ret.remaining(), 0);

            // write 1/2 items again
            let (result, ret) = tx.write(vec![3, 4]).await;
            assert_eq!(result, StreamResult::Complete(2));
            assert_eq!(ret.remaining(), 0);

            // write to a dropped stream
            let (result, ret) = tx.write(vec![0]).await;
            assert_eq!(result, StreamResult::Dropped);
            assert_eq!(ret.remaining(), 1);
        };
        let ((), ()) = futures::join!(test, read_stream(rx));

        // With no prior transfers, closing the reader reports Dropped(0).
        let (mut tx, rx) = wit_stream::new::<u8>();
        drop(rx);
        let (result, ret) = tx.write(vec![5, 6]).await;
        assert_eq!(result, StreamResult::Dropped);
        assert_eq!(ret.remaining(), 2);
        let (result, ret) = tx.write_buf(ret).await;
        assert_eq!(result, StreamResult::Dropped);
        assert_eq!(ret.into_vec(), vec![5, 6]);
        assert_eq!(tx.write_all(vec![7, 8]).await, vec![7, 8]);

        // An empty stream latches EOF before another read or next call.
        let (tx, mut rx) = wit_stream::new::<u8>();
        drop(tx);
        for _ in 0..2 {
            let (result, buf) = rx.read(Vec::with_capacity(1)).await;
            assert_eq!(result, StreamResult::Dropped);
            assert!(buf.is_empty());
        }
        assert_eq!(rx.next().await, None);
    }
}
