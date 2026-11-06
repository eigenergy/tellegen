---
'@tellegen/engine': patch
---

Generating a grid over a long, thin region takes less memory. The fill that spaces out the generated buses kept a bucket for every cell of the region's bounding box, most of them empty and outside the region. It now keeps buckets only for occupied cells. The same request and seed generate the same grid as before.
