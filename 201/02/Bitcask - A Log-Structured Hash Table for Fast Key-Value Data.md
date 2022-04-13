[原文地址](https://github.com/basho/bitcask/blob/develop/doc/bitcask-intro.pdf)

Bitcask 的起源与 Riak 分布式数据库有关。在 Riak key/value 集群中，每个节点都使用可插拔的本地存储；几乎任何 k/v-shaped 的都可以用作为主机的存储引擎。这种可插拔性使 Riak 上的进展得以并行化 - 改进和测试存储引擎不会影响代码库的其余部分。

许多本地 key/value 存储已经存在，包括但不限于 Berkeley DB、Tokyo Cabinet 和 Innostore。在评估此类存储引擎时，我们寻求的指标有很多、诸如：
- 读写低延迟
- 高吞吐，特别是在写入incoming stream of random items
- 处理比 RAM 大得多的数据集的能力（不降级）
- 崩溃友好 - 快速恢复 & 不丢数据
- 易于备份和恢复
- 一种相对简单、可理解（因而可支持）的代码和数据结构
- 在高访问负载或大容量下的可预测行为
- 允许在 Riak 中轻松使用的许可证

实现以上部分是容易的、但是都实现就很麻烦了

就上述所有目标而言，所有可用的本地 KV 存储系统（包括但不限于作者编写的系统）都不理想。当我们与 Eric Brewer 讨论这个问题时，他对 hash table log merging 有一个洞见：这样做可能与 LSM 树一样快或更快。

这使我们从一个新的角度探索了在 20 世纪 80 年代和 90 年代首次开发的 [[The Design and Implementation of a Log-Structured File System|LFS]] 中使用的一些技术。这一探索导致了 Bitcask 的开发，这是一种非常好地满足上述**所有目标**的存储系统。Bitcask 最初的开发目标是在 Riak 下使用，但它是通用的，也可以作为其他应用程序的本地 KV 存储。

我们最终使用的模型在概念上非常简单。Bitcask 实例是一个目录，我们强制要求在特定时间只有一个进程可打开该 Bitcask 进行写入。你可以将该进程地视为“数据库服务器”。在任何时候，该目录中都有一个文件处于“活动”状态，供服务器写入。当该文件达到大小阈值时，它将被关闭，并创建一个新的活动文件。一旦一个文件被关闭，无论是有意关闭还是由于服务器退出，它都被认为是不可变的，永远不会被再次打开进行写入。


A bitcask in disk ![图1](/static/bitcask_f1.png):

"活动"文件仅通过追加写入，这意味着顺序写入不需要磁盘查找。为每个 K/V entry 编写的格式很简单：
![图2](/static/bitcask_f2.png)
> CRC - 循环冗余校验

每次写入时，活动文件都会出现一个新条目。请注意，删除只是写入一个特殊的墓碑值，它将在下次合并时被删除。因此，这些数据项都是线性的：
![图3](/static/bitcask_f3.png)

追加完成后，一个名为“keydir”的内存结构将被更新。keydir 只是一个哈希表，它将 Bitcask 中的每个键映射到一个固定大小的结构，给出该键最近写入的项的文件、偏移量和大小。
![图4](/static/bitcask_f4.png)

当写入发生时，keydir 会自动更新最新数据的位置。旧数据仍然存在于磁盘上，但任何新的读取都将使用 keydir 中可用的最新版本。正如我们稍后将看到的，合并过程最终将删除旧值。

读取一个值很简单，而且只需要一次磁盘搜索。我们在 keydir 中查找 key，然后从那里使用该查找返回的 file_id、position 和 size 读取数据。在许多情况下，操作系统的 read-ahead cache 使这一操作比预期的要快得多。
![图5](/static/bitcask_f5.png)

随着时间的推移，这个简单的模型可能会占用大量空间，因为我们只是写入新数据，但没有动老数据。我们使用“merging”的压缩过程解决这个问题。merging 通过遍历 Bitcask 中的所有非活动（不可变）文件，并输出一组仅包含每个 key 的 live / latest version 的 data files。

完成后，我们还会在每个 data file 旁创建一个“hint file”。它们本质上类似于数据文件，但不包含数据，而是包含相应 data file 中 value 的 position 和 size。
![图6](/static/bitcask_f6.png)

当 Erlang 进程打开 Bitcask 时，它会检查同一个 VM 中是否已有另一个 Erlang 进程"使用"该 Bitcask :
- 如果是这样，它将与该进程共享 keydir
- 如果没有，它会扫描目录中的所有数据文件，以构建新的 keydir（对于任何包含 hint file 的 data file，都将扫描该文件，以获得更快的启动时间）

这些基本操作是 bitcask 系统的精髓。显然，我们并没有试图在本文档中公开操作的每一个细节；我们的目标是帮助您了解 Bitcask 的一般机制。

Some additional notes on a couple of areas we breezed past are probably in order(??? 没太懂这句话):
- 我们提到过使用 OS 的文件系统的 cache 可以提高读性能。我们也讨论过要不要在 bitcask 内部也添加一个读 cache、但是还不清楚这个的工作量以及带来的收益有多少。
- 我们将很快的针对各种 API 类似的本地存储系统做基准测试。然而，Bitcask 的初初心并不是成为最快的存储引擎，而是在有“足够”的速度前提下，代码、设计和文件格式都是高质量和简单的。在我们最初的简单基准测试中，我们看到 Bitcask 在许多情况下都轻松地优于其他快速存储系统。
- 对于大多数"局外人"来说，对一些最难实现的细节也是最不感兴趣的，因此我们在这篇简短的文档中没有介绍（例如）internal keydir locking scheme。
- Bitcask 不执行任何数据压缩，因为这样做的成本/收益因应用程序而异。


让我们看看我们出发时的目标：
- **low latency per item read or written**：Bitcask 很快。我们计划很快进行更全面的基准测试，在我们的早期测试中，sub-millisecond typical median latency (and quite adequate higher percentiles)），我们相信可以实现我们的速度目标。
- **high throughput, especially when writing an incoming stream of random items**：在使用低速磁盘的笔记本电脑上进行的早期测试中，我们已看到每秒 5000-6000 写入的吞吐量。
- **ability to handle datasets much larger than RAM w/o degradation**：上述测试在所讨论的系统上使用了一个超过 10×RAM 的数据集，并且没有表现出有行为改变。鉴于 Bitcask 的设计，这与我们的预期一致。
- **crash friendliness, both in terms of fast recovery and not losing data**：由于 data files 和 commit log 在 Bitcask 中是相同的，因此恢复非常简单，不需要“replay”，hint file可用于加快启动过程。
- **ease of backup and restore**：由于文件在 rotation 后是不可变的，可以使用操作系统易用的任何系统级机制来轻松备份。恢复只需要将数据文件放在目标目录中即可。
- **a relatively simple, understandable (and thus supportable) code structure and data format**：Bitcask 概念简单、代码清晰，data file 非常容易理解和管理。我们很乐意支持一个放置在 Bitcask 上的系统。
- **predictable behavior under heavy access load or large volume**：在高访问负载下，我们已经看到 Bitcask 表现良好。到目前为止，它的容量只有两位数的千兆字节，但我们很快就会用更多的数据对其进行测试。Bitcask 是这样的，我们不希望它在更大的容量下表现出太大的不同，但有一个可预测的例外，即 keydir 结构随着 key 数量的增加而少量增长，并且必须完全存放在 RAM。这种限制在实践中是很小的，因为即使使用了数百万个密钥，其内存也低于 GB。

总之，考虑到这一系列具体目标，Bitcask 比我们现有的任何产品都更适合我们的需求。

API 非常简单：
![图7](/static/bitcask_f7.png)
