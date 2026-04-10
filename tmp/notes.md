# HTTP over TCP in one line

- Listener on a TCP port that collects messages from stream and follows HTTP(rotocol)

# TCP

- Ya read it 8 bytes at a time just like any filereader builtin
- TCP guarantees data is receiver in-order
- Sliding window of n packets, when one (any or first in window?) is acknowledged by receiver, move sliding window by one and send new package out
  - How does the idea of the "TCP handshake" connect to this? Is this the handshake?
  - How does this solve sorting within the sliding window (How does any packet from 0..n received by user/server know where in the sliding window its index should be? Is there some kind of index/id stored in each 8 bytes package?)
- vs UDP
  - UDP doesn't have a sliding window as clamp and doesn't wait for receiver acknowledgement
    - Sends everything at once
  - No 'builtin' mechanism for detecting missing packets (sliding window)
    - Negative acknowledge (package missing)

# HTTP

- Protocol on top of TCP that
  - Uses /r/n
  - Request
    - Method (GET/POST/ETC)
    - Location (/index.html)
    - HTTP version
  - Field-names/headers (flesh out)
    - Host
    - Auth
    - Accept (Only filetype stuff like 'image\\\*'?)
    - User-agent (user identifiers)
    - Content-type (application/json, etc)
    - Content-length in bytes (or chunked stuff)
  - Ends metadata 'segment' with '/r/n'
- HTTP/1.1 vs 2/3
  - Hpack/QPack (look into)

# TODO

- https//www.rfc-editor.org/rfc/rfc9110.html
- https//www.rfc-editor.org/rfc/rfc9112.html
