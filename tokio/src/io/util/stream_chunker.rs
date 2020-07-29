use crate::io::AsyncRead;
use crate::stream::Stream;
use bytes::{Bytes, BytesMut};
use pin_project_lite::pin_project;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Convert an [`AsyncRead`] into a [`Stream`] of byte chunks.
    ///
    /// This type is usually created using the [`stream_chunker`] function.
    ///
    /// [`AsyncRead`]: crate::io::AsyncRead
    /// [`Stream`]: crate::stream::Stream
    /// [`stream_chunker`]: crate::io::stream_chunker
    #[derive(Debug)]
    #[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
    pub struct StreamChunker<R> {
        #[pin]
        inner: Option<R>,
        buffer: BytesMut,
    }
}

/// Convert an [`AsyncRead`] into a [`Stream`] of byte chunks.
///
/// [`AsyncRead`]: crate::io::AsyncRead
/// [`Stream`]: crate::stream::Stream
///
/// This is often used together http crates such as hyper and reqwest, as those
/// crates want a `Stream` as the body of the request. This function can be used
/// to create such a `Stream`.
///
/// # Example
///
/// ```
/// use std::io::Cursor;
/// use tokio::io::stream_chunker;
/// use tokio::stream::StreamExt;
/// # #[tokio::main]
/// # async fn main() -> std::io::Result<()> {
///
/// let cursor = Cursor::new(b"Hello world.");
///
/// let mut chunks = stream_chunker(cursor);
///
/// let mut combined = Vec::new();
/// while let Some(chunk) = chunks.next().await {
///     combined.extend_from_slice(&chunk?);
/// }
///
/// assert_eq!(combined, b"Hello world.");
///
/// # Ok(())
/// # }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
#[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
pub fn stream_chunker<R>(async_read: R) -> StreamChunker<R>
where
    R: AsyncRead,
{
    StreamChunker::new(async_read)
}


impl<R> StreamChunker<R>
where
    R: AsyncRead,
{
    /// Convert the provided `AsyncRead` into a `Stream`.
    fn new(async_read: R) -> Self {
        Self {
            inner: Some(async_read),
            buffer: BytesMut::with_capacity(2048),
        }
    }
}

impl<R> Stream for StreamChunker<R>
where
    R: AsyncRead,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        this.buffer.reserve(128);

        let mut inner = match this.inner.as_mut().as_pin_mut() {
            Some(inner) => inner,
            None => {
                return Poll::Ready(None);
            },
        };

        loop {
            match inner.as_mut().poll_read_buf(cx, this.buffer) {
                Poll::Pending => {
                    if this.buffer.len() == 0 {
                        return Poll::Pending;
                    } else {
                        return Poll::Ready(Some(Ok(this.buffer.split().freeze())));
                    }
                },
                Poll::Ready(Err(err)) => {
                    this.inner.set(None);
                    return Poll::Ready(Some(Err(err)));
                },
                Poll::Ready(Ok(0)) => {
                    this.inner.set(None);
                    if this.buffer.len() == 0 {
                        return Poll::Ready(None);
                    } else {
                        return Poll::Ready(Some(Ok(mem::take(this.buffer).freeze())));
                    }
                },
                Poll::Ready(Ok(_)) => {
                    // around loop again
                },
            }
        }
    }
}
