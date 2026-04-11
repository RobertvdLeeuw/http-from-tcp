# HTTP over TCP in one line

- Listener on a TCP port that collects message chunks from stream and follows HTTP(rotocol)

# Quick notes

- Use curl and browser to test stuff

# TCP

- Ya read it a couple bytes at a time just like any filereader builtin
- TCP guarantees data is receiver in-order
- Sliding window of n packets, when one (first in window, oldest unacknowledged byte) is acknowledged by receiver, move sliding window by one and send new package out
- TCP handshake: SYN -> SYN-ACK -> ACK
- Adds sequence number to each packet for re-ordering by receiver
-
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
    - Accept (Only filetype stuff like 'image\\\*', wildcards, and quality values (preference/priority))
    - User-agent (user identifiers)
    - Content-type (application/json, etc)
    - Content-length in bytes (or chunked stuff)
      - If no Content-length, just assume no body
  - Ends metadata 'segment' with '/r/n/r/n' (empty line)
- HTTP/1.1 vs 2/3
  - Hpack/QPack (look into)

# Code specific stuff

- Definitely a request struct
  - How can I keep that from being mutable during it's entire use?
  - Mutability should only be in chunk reconstructing phase, before or during parsing phase

# TODO

- https//www.rfc-editor.org/rfc/rfc9110.html
- https//www.rfc-editor.org/rfc/rfc9112.html
- During start of development, figure out right time(s) to implement testing
