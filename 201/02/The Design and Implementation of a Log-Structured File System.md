[论文地址](https://people.eecs.berkeley.edu/~brewer/cs262/LFS.pdf)

# 摘要
log-structured file system: 一种用来管理磁盘存储的技术。其将所有操作都以 log-like 的结构顺序写入磁盘，从而提高了写入与故障恢复的效率。

磁盘将只会存储这些 log，其包含了索引信息（故读取是高效的）。

为了在磁盘上保持较大的空闲区域，以便快速写入。我们将日志分成多个段（segment），and use a segment cleaner to compress the live information from heavily fragmented segments（#question 这里表达的是将碎片化严重的段上的存活信息进行干嘛？compress？）.

我们提供了一系列模拟，证明了基于成本和效益的简单回收策略的效率。我们已经实现了一个名为 Sprite LFS 的原型日志结构文件系统；在小文件写入方面，它的性能比当前的 Unix 文件系统高出一个数量级，而在读取和大型写入方面，它的性能与 Unix 相当或超过 Unix。即使包括清理开销，Sprite LFS 也可以使用 70%的磁盘带宽进行写入，而 Unix 文件系统通常只能使用 5-10%。

# 1. 介绍
CPU 的发展速度远快于磁盘。

通过发明 LFS 我们是为了比现有的 file system 更大的发挥磁盘的效率。

LFS 基于这样一种假设：文件缓存在主内存中，增加内存大小将使缓存在满足读取请求方面越来越有效。因此，磁盘流量主要是写操作。LFS 以称为 log 的顺序结构将所有新信息写入磁盘（LFS writes all new information to disk in a sequential structure called the log）。这种方法通过消除几乎所有的寻道，显著提高写性能。log 的连续性特征还可更快的进行崩溃恢复：当前的 Unix 文件系统通常必须扫描整个磁盘，以在崩溃后恢复一致性，但日志结构的文件系统只需要检查日志的最新部分。

日志记录的概念并不新鲜，最近的一些文件系统都将日志作为辅助结构，以加快写入和崩溃恢复[2,3]。然而，这些其他系统仅将日志用于临时存储；永久存储到磁盘的方式还是传统的随机存取结构。相比之下，LFS 将数据永久存储在日志中：磁盘上没有其他结构。该日志包含索引信息，读回文件（read back files[^a]）的效率与当前文件系统的效率相当。

为了让 LFS 高效运行，它必须确保始终有大量可用空间用于写入新数据。这是日志结构文件系统设计中最困难的挑战。在本文中，我们提出了一种基于称为段(segment)的大范围的解决方案，其中段清理程序通过压缩严重碎片段中的存活数据来不断地重新生成空段。我们使用了一个模拟器来探索不同的清洗策略，并发现了一种基于成本和效益的简单但有效的算法：对于较旧、变化较慢的数据与年轻、变化迅速的数据分开，并在清洗过程中对其进行不同的处理。

我们已经构建了一个名为 Sprite LFS 的 LFS 原型系统，并已作为 Sprite 网络操作系统的组件被生产所使用。基准测试程序表明，对于小文件，Sprite LFS 的原始写入速度比 Unix 快一个数量级以上。即使其他工作负载，例如包括读取和大文件访问的工作负载，Sprite LFS 在所有情况下都至少与 Unix 一样快，只有一种情况除外（文件在随机写入后按顺序读取）。我们还测量了生产系统中回收的长期开销。总的来说，Sprite LFS 允许磁盘原始带宽的 65-75%用于写入新数据（其余用于清理）。相比之下，Unix 系统只能利用磁盘原始带宽的 5-10%来写入新数据；剩下的时间都在寻道(seeking).

本文的其余部分分为六节。第 2 节回顾了 20 世纪 90 年代设计计算机文件系统中遇到过的问题。第 3 节讨论了 LFS 的设计方案，并推导了 Sprite LFS 的结构，特别侧重于清理机制。第 4 节介绍了 Sprite LFS 的崩溃恢复系统。第 5 节使用基准和长期测量清理成本来评估 Sprite LFS（evaluates Sprite LFS using benchmark programs and long-term measurements of cleaning overhead）。第 6 节将 Sprite LFS 与其他文件系统进行比较，第 7 节总结。

# 2. Design for ﬁle systems of the 1990’s
文件系统设计由两种主要力量控制：技术（提供基本构建块）和工作负载（确定必须高效执行的一组操作）。

本节总结了正在进行中的技术，并描述了它们对文件系统设计的影响。还描述了影响 Sprite LFS 设计的工作负载，并展示了当前的文件系统如何不能应对工作负载和技术的变化。

## 2.1. Technology
有三种技术对文件系统设计特别重要：处理器、磁盘和主内存。
- 处理器之所以重要，是因为它们的速度正以接近指数的速度增长，而且这种改进似乎可能会持续到 20 世纪 90 年代的大部分时间。这也给计算机系统的所有其他元素带来了加速的压力，从而使系统不会变得不平衡。
- 磁盘技术也在迅速改进，但改进主要集中在成本和容量方面，而不是性能方面。磁盘性能有两个组成部分：传输带宽(transfer bandwidth)和访问时间(access time)。尽管这两个因素都在改善，但改善的速度比 CPU 速度慢得多。使用磁盘阵列和平行磁头磁盘可以大幅提高磁盘传输带宽，但访问时间似乎不太可能有重大改善（这取决于难以改善的机械运动）。如果一个应用程序导致一系列由 seek 分隔的小磁盘传输，那么即使使用更快的处理器，该应用程序在未来十年内也不太可能经历太多的加速。
- 主存储器，它的大小正以指数级的速度增长。现代文件系统将最近使用的文件数据缓存在主内存中，更大的主存储器使更大的文件缓存成为可能。这对文件系统行为有两个影响。
	- 更大的文件缓存通过吸收更大比例的读取请求，改变了呈现给磁盘的工作负载。大多数写请求为了安全起见最终必须落盘，因此磁盘流量（和磁盘性能）将越来越多地由写操作控制。
	- 更大的文件缓存可以用作写缓冲区，在将任何修改的块写入磁盘之前，可以收集大量修改的块。缓冲可使写入块更有效成为可能，例如，通过仅使用一次寻道在单个顺序传输中写入所有块。当然，写缓冲的缺点是会增加崩溃期间丢失的数据量。在本文中，我们假设崩溃很少发生，每次崩溃损失几秒钟或几分钟的工作是可以接受的；对于需要更好的崩溃恢复的应用程序，非易失性 RAM 可用于写入缓冲区。

要点总结：
1. 出于机械运动可优化性不太高，磁盘性能增长远不如处理器
2. 主存储器越来越大，故基本读都可以命中主存储器缓存，处于安全性考虑、写操作才需要落盘，故磁盘流量将主要取决于写操作
3. 本文采用写操作先在主存储器缓存、而后批量写入的写优化。并假设可以接收崩溃带来的数据丢失

## 2.2. Workloads
计算机应用程序中常见几种不同的文件系统工作负载。文件系统设计要有效处理的最困难的工作负载之一是在办公室和工程环境中。办公室和工程应用往往以访问小文件为主；一些研究测量的平均文件大小只有几千 KB。小型文件通常会导致小型的随机磁盘 I/O，此类文件的创建和删除时间通常通过更新文件系统的“元数据”（用于定位文件属性和数据块的数据结构）来控制。

顺序访问大文件（如超级计算环境）的工作负载也会带来有趣的问题，但对文件系统软件来说并非如此。有许多技术可以确保这些文件在磁盘上按顺序排列，因此 I/O 性能往往受到 I/O 和内存子系统带宽的限制，而不是文件分配策略的限制。

在设计 LFS 时，我们决定将重点放在小文件访问的效率上，并将提高大文件访问带宽问题留给硬件设计师。

幸运的是，Sprite LFS 中使用的技术适用于大文件和小文件。

要点总结：访问大文件和小文件的工作负载是不同的，但访问大文件的瓶颈不在文件系统上。故 Sprite LFS 的思想同时适用于大/小文件的访问负载

## 2.3. Problems with existing ﬁle systems
当前的文件系统存在两个普遍问题，这使得它们很难应对 20 世纪 90 年代的技术和工作负载:
1. 首先，它们在磁盘上传播信息的方式会导致太多的小访问。例如，伯克利 Unix 快速文件系统（Unix FFS）在磁盘上按顺序排列每个文件时非常有效，但它在物理上分隔了不同的文件。此外，文件属性（“inode”[^2]）与文件内容是分开的，包含文件名的目录条目也是分开的。在 Unix FFS 中创建新文件至少需要五个独立的磁盘 I/O，每个磁盘 I/O 前面都有一个 seek：对文件属性的两种不同访问，以及对文件数据、目录数据和目录属性的一种访问。在这样的系统中写入小文件时，用于写新数据的磁盘带宽占不到 5%；剩下的时间都用来寻道。
2. 第二个问题是它们倾向于同步写：应用程序必须等待写入完成，而不是在后台处理写入时继续。例如，即使 Unix FFS 异步写入文件数据块，文件系统元数据结构（如目录和索引节点）也是同步写入的。对于包含许多小文件的工作负载，磁盘流量主要由同步元数据写入控制。同步写入将应用程序的性能与磁盘的性能耦合，使应用程序很难从更快的 CPU 中获益。它们还阻止了文件缓存作为写缓冲区的潜在用途。不幸的是，像 NFS 这样的网络文件系统还引入了额外的同步行为。其简化了崩溃恢复，但降低了写入性能。

尽管在本文中我们使用 Berkeley Unix 快速文件系统（Unix FFS）作为当前文件系统设计的示例，并将其与 LFS 进行比较。之所以使用 Unix FFS，是因为它在文献中有很好的记录，并在几种流行的 Unix 操作系统中使用。但本节中介绍的问题不是 Unix FFS 独有的，可在大多数其他文件系统中找到。

# 3. Log-structured ﬁle systems
LFS 的基本思想是通过在文件缓存中缓冲一系列文件系统更改，然后在单个磁盘写入操作中按顺序将所有更改写入磁盘，从而提高写入性能。

写入操作中写入磁盘的信息包括文件数据块、属性、索引块、目录，以及几乎所有用于管理文件系统的其他信息。对于包含许多小文件的工作负载，LFS 将传统文件系统的许多小同步随机写入转换为大型异步顺序传输，可以利用近 100%的原始磁盘带宽。

虽然 LFS 的基本思想很简单，但要兑现 logging 的潜在好处须解决两个关键问题：
1. 第一个问题是如何从 log 中检索信息；这是下文第 3.1 节的主题。
2. 第二个问题是如何管理磁盘上的可用空间，以便始终提供大范围的可用空间用于写入新数据。这是一个更加困难的问题；这是第 3.2-3.6 节的主题。

表 1 总结了 Sprite LFS 用于解决上述问题的磁盘数据结构；数据结构将在本文后面的章节中详细讨论。

--- 

| Data structure       | Purpose                                                                                | Location | Section |
| -------------------- | -------------------------------------------------------------------------------------- | -------- | ------- |
| Inode                | Locates blocks of file, holds protection bits, modify time,etc.                        | Log      | 3.1     |
| Inode map            | Locates position of inode in log, holds time of last access plus version number.       | Log      | 3.1     |
| Indirect block       | Locates blocks of large files.                                                         | Log      | 3.1     |
| Segment summary      | Identifies contents of segment (file number and offset for each block).                | Log      | 3.1     |
| Segment usage table  | Counts live bytes still left in segments,stores last write time for data in segments.  | Log      | 3.2     |
| Superblock           | Holds static configuration information such as number of segments and segment size.    | Fixed    | None    |
| Checkpoint region    | Locates blocks of inode map and segment usage table,identifies last checkpoint in log. | Fixed    | 4.1     |
| Directory change log | Records directory operations to maintain consistency of reference counts in inodes.    | Log      | 4.2     |

For each data structure the table indicates the purpose served by the data structure in Sprite LFS. The table also indicates whether the data structure is stored in the log or at a ﬁxed position on disk and where in the paper the data structure is discussed in detail. Inodes, indirect blocks, and superblocks are similar to the Unix FFS data structures with the same names. Note that Sprite LFS contains neither a bitmap nor a free list.


## 3.1 File location and reading
虽然术语“日志结构”可能表示需要顺序扫描才能从日志中检索信息，但在 Sprite LFS 中并非如此。我们的目标是达到或超过 Unix FFS 的读性能。为了实现这一目标，Sprite LFS 在日志中输出索引结构，以允许随机访问检索。Sprite LFS 使用的基本结构与 Unix FFS 中使用的基本结构相同：每个文件都有一个名为 inode 的数据结构，其中包含文件的属性（类型、所有者、权限等）以及文件前十个 blocks 的磁盘地址；对于大于 10 个 blocks 的文件，inode 还包含一个或多个 indirect blocks 的磁盘地址，每个 indirect block 包含更多数据或 indirect block的地址。一旦找到文件的 inode，在 Sprite LFS 和 Unix FFS 中读取文件所需的磁盘 I/O 次数是相同的。

在 Unix FFS 中，每个 inode 都位于磁盘上的固定位置；给定文件的标识号，一个简单的计算就可以得到文件 inode 的磁盘地址。相比之下，Sprite LFS 不会在固定位置放置 inode；它们被写入日志。Sprite LFS 使用名为 inode map 的数据结构来维护每个 inode 的当前位置。给定文件的标识号后，必须对 inode map 索引后才能确定 inode 的磁盘地址（Given the identifying number for a ﬁle, the inode map must be indexed to determine the disk address of the inode.）。inode map 被划分为 blocks 后写入 log；每个磁盘上有个的固定的 checkpoint region 记录着所有 inode map block 的位置。幸好的是，inode map 非常紧凑，可以将活跃部分完全缓存在主内存中：inode map 查找基本不需要访问磁盘。

图 1 显示了在不同目录中创建两个新文件后，Sprite LFS 和 Unix FFS 中的磁盘布局。虽然这两种布局具有相同的逻辑结构，但 LFS 产生了更紧凑的布局。因此，Sprite LFS 的写入性能比 Unix FFS 好得多，而读取性能也一样好。

![图1](/static/LFS_figure1.png)
This example shows the modiﬁed disk blocks written by Sprite LFS and Unix FFS when creating two single-block ﬁles named dir1/file1 and dir2/file2. Each system must write new data blocks and inodes for file1 and file2, plus new data blocks and inodes for the containing directories. Unix FFS requires ten non-sequential writes for the new information (the inodes for the new ﬁles are each written twice to ease recovery from crashes), while Sprite LFS performs the operations in a single large write. The same number of disk accesses will be required to read the ﬁles in the two systems. Sprite LFS also writes out new inode map blocks to record the new inode locations.

## 3.2. Free space management: segments
LFS 最困难的设计是可用空间的管理。其目标是为写入新数据维护较大的空闲数据块。最初，所有可用空间都在磁盘上的一个区段中，但当日志到达磁盘的末尾时，可用空间将被分割成许多小区段，因为发生了文件的删除、覆盖。

对于这种情况，文件系统有两个选择：threading 和 copying。如图 2 所示。
1. 第一种是不动还存活的 blocks and thread the log through the free extents[^3]（#ques 这里的 thread the log through the free extents 是什么意思？）. 不幸的是，threading 会导致空闲空间碎片化严重，最后导致大块的持续写入变得不可能，这样（设计）的话，LFS 就不会比传统 FS 快了。
2. 第二种方案是将 live data 拷贝出来、从而能留下更大的 extends[^3]给后续写入. 这里我们假设拷贝出来的 live data 会被压缩并写入到头部（当然现实中也存在别的情况）。这样做的劣势是 copying 的开销，特别是对于长时间存活的数据 - 可能会被无限次的拷贝（对于 circularly 写入磁盘的情况，长时间存活的数据需要周期性的被复制）。

![图2](/static/LFS_figure2.png)
注意，图中白色的是没有数据（被删除）过的 blocks 。
在 LFS 中，可以通过 copying 旧块或在旧块周围 threading the log 来生成日志的可用空间。
- 图左侧显示了 threaded log 方法，其跳过 active blocks 并覆盖已删除或覆盖的 blocks。Pointers between the blocks of the log are maintained so that the log can be followed during crash recovery
- 图右侧显示了 copying 方案，where log space is generated by reading the section of disk after the end of the log and rewriting the active blocks of that section along with the new data into the newly generated space.

Sprite LFS **结合**了 threading 和 copying。其将磁盘被划分为称为 segment 的固定大小的大 extend。任何给定的 segment 都是从开始到结束按顺序写入的，在重写 segment 之前，必须将所有 live data 从 segment 中复制出来。然而，the log is threaded on a segment-by-segment basis；如果系统可以将长期存活的数据收集到一起，形成 segment，则可以跳过这些 segment，这样就避免了重复复制数据。选择的 segment 大小足够大，以至于读取或写入整个段所需的传输时间远远大于从寻道 segment 开始的成本。这使得整个 segment 操作可以几乎占满磁盘的全部带宽，而不用管访问 segment 的顺序。Sprite LFS 目前使用的 segment 大小为 512 KB 或 1 MB。

## 3.3. Segment cleaning mechanism
从 segment 中复制 live data 的过程称为段清理。在 Sprite LFS 中，这是一个简单的三步过程：将多个 segment 读入内存，识别 live data，然后将 live data 写回较少的干净片段。此操作完成后，读取的段被标记为干净，可以用于新数据或额外的清理。

既然涉及到数据 block 的移动，那么首先需要能够知道 segment 上哪些 block 是 live 的、其次还需要知道 block 所属的 file（因为需要更新 file 的 inode 来指向 block 新写的位置）。Sprite LFS 通过**在每个 segment 中编写一个 segment summary block**来解决这两个问题。摘要块标识写入段中的每一条信息；例如，对于每个 file data block，摘要块包含该 block 的 file number 和 block number。在需要多个 log 写入才能填满 segment 的时候，Segments 可有多个 segment summary blocks。(Partial-segment writes occur when the number of dirty blocks buffered in the ﬁle cache is insufﬁcient to ﬁll a segment.)。段摘要块在编写过程中几乎没有开销，它们在崩溃恢复（参见第 4 节）以及清理过程中非常有用。

Sprite LFS 还使用 segment summary block 来区分活动块和被覆盖或删除的块。一旦知道一个块的标识，就可以通过检查文件的 inode 或 indirect block 来是否还在引用这个 block 来确定其活动性，如果是的话，那么这个 block 是活的；如果没有，那么这个 block 就死了。Sprite LFS 通过在每个文件的 inode map 中保留一个版本号来略微优化该检查；每当文件被删除或 truncate 时，版本号就会增加。版本号与 inode number 组合在一起构成文件内容的唯一标识符（uid）。 segment summary block 记录段中每个 block 的 uid；如果清除段时块的 uid 与当前存储在 inode map 中的 uid 不匹配，则可以立即丢弃该 block，而无需检查文件的 inode。

这种清理方法意味着 Sprite 中没有 free-block list/bitmap。除了节省内存和磁盘空间外，消除这些数据结构还简化了故障恢复。如果存在这些数据结构，则需要额外的代码来记录对结构的更改，并在崩溃后恢复一致性。

## 3.4. Segment cleaning policies
鉴于上述基本机制，还存在四个问题：
1. segment cleaner 应该在什么时候执行？一些可能的选择是，它在后台以低优先级连续运行，或者仅在夜间运行，或者仅在磁盘空间几乎耗尽时运行。
2. 一次应该清理多少个 segments？清理提供了重新组织磁盘上数据的机会；一次清理的片段越多，重新排列的机会就越多。
3. 哪些 segments 应该被清理？一般来说、碎片化严重的 segments 应该被清理、但是实际上这并非最佳选择。
4. 如何重新组织 live blocks？一种是基于空间局部性，比如将属于同一个目录的 files 写入到同一个 segment。另一种是基于时间局部性，比如按照 blocks 上次修改的时间排序，然后将时间靠近的 blocks 放入同一个 segment。

到目前为止，我们没有系统地解决上述前两项问题。当回收 segment 数降至阈值以下（通常为几十）时，Sprite LFS 开始回收。它一次回收几十个，直到回收的数量超过另一个阈值（通常为 50-100 个段）。Sprite LFS 的整体性能似乎对阈值的精确选择不太敏感。相比之下，第三和第四个策略决策至关重要：根据我们的经验，它们是决定日志结构文件系统性能的主要因素。第 3 节的其余部分将讨论我们对要清理哪些 segments 以及如何对 live data 进行分组的分析。

我们使用一个称为 write cost 的术语来比较清洗策略。write cost 是指每写入一字节新数据，磁盘处于繁忙状态的平均时间，包括所有清理开销。write cost 表示为如果没有清理开销，并且可以在没有寻道时间或旋转延迟的情况下以全带宽写入数据所需时间的倍数。写入成本为 1.0 是完美的：这意味着新数据可以以完整的磁盘带宽写入，并且没有清理开销。写入成本为 10 意味着只有磁盘最大带宽的十分之一用于写入新数据；其余的磁盘时间用于查找、轮换延迟或清理。

对于有大 segments 的 LFS，无论是在写入还是清理时，寻道延迟和旋转延迟都可以忽略不计，因此 write cost = 移动出磁盘的总字节数 / 新数据的字节数。这一成本由被清理的 segments 中的利用率（数据仍然有效的部分）决定。**在稳定状态下，回收器必须为写入的每段新数据提供一个干净段**。要做到这一点，它读取 N 个完整的数据段，并写出 N*u 个实时数据段（其中 u 是这些数据段的利用率，0）≤ u<1）。这就产生了 N*（1−u） 用于新数据的连续可用空间。故：
![公式1](/static/LFS_formula1.png)


在上述公式中，我们做出了保守的假设，即必须完整读取一个段，才能 recover the live blocks；实际上，只读取 live blocks 可能会更快，尤其是在利用率非常低的情况下（我们还没有在 Sprite LFS 中尝试过）。如果要清理的 segment 没有 live blocks（u=0），则根本不需要读取它，写入成本为 1.0。

图 3 显示了 write cost 与 u 的函数关系。作为参考，小文件工作负载上的 Unix FFS 最多使用 5-10%的磁盘带宽，写入成本为 10-20（具体测量请参见第 5.1 节中的[11]和图 8）。通过日志记录、延迟写入和磁盘请求排序，这可能会提高到大约 25%的带宽[12]，或 4%的写入成本。图 3 表明，为了使 LFS 能够从当前的 Unix FFS 输出，已清理的 segement 的利用率必须小于 0.5 才能优于改进的 Unix FFS。

![图3](/static/LFS_figure3.png)
在 LFS 中， write cost 很大程度上取决于已清理的段的利用率。清理的数据段中的 live data 越多，清理所需的磁盘带宽就越多，无法用于写入新数据。该图还显示了两个参考点：“今天的 FFS”代表今天的 Unix FFS，以及“改进的 FFS”，这是我们对改进的 Unix FFS 可能的最佳性能的估计。Unix FFS 的写入成本对使用的磁盘空间量不敏感。

需要注意的是，上面讨论的利用率并不是 live data 占磁盘的总比例；而是在 segments 上被清理掉的 live blocks 部分。文件使用的变化会导致某些 segment 的利用率低于其他 segment，回收器可以选择利用率最低的 segment 进行清理；这些的利用率将低于磁盘的总体平均水平。

即便如此，日志结构文件系统的性能也可以通过降低磁盘空间的总体利用率来提高。随着磁盘被使用的较少，被清理的段将具有较少的活动块，从而降低 write cost。LFS 提供了一种 cost-performance 权衡：如果磁盘空间未充分利用，可以实现更高的性能，但每可用字节的成本很高；如果磁盘容量利用率提高，存储成本和性能都会降低。性能和空间利用率之间的这种折衷并不是 LFS 所独有的。例如，Unix FFS 只允许文件占用 90%的磁盘空间。剩余的 10%是预留给空间分配算法以用于其的高效运行。

在 LFS 中，以低成本实现高性能的关键是强制磁盘进入双峰段分布（bimodal segment distribution）: 其中大多数段几乎已满，少数段为空或几乎为空，而回收器几乎总是处理那些空段。这使得总体磁盘容量利用率较高，但写入成本较低。以下部分介绍了我们如何在 Sprite LFS 中实现这种双峰分布。

## 3.5. Simulation results
我们构建了一个简单的文件系统模拟器，以便在受控条件下分析不同的回收策略。模拟器的模型没有参考‌选择实际的文件系统使用模式（模型比实际情况严格得多），但它帮助我们了解随机访问模式和空间局部性的影响，这两种都可以用来降低回收成本。模拟器将文件系统建模为固定数量的 4-kbyte 大小的文件，所选数量取决于产生特定的磁盘总体容量利用率。在每一步中，模拟器用新数据覆盖其中一个文件，使用以下两种伪随机访问模式中的一种：

| 伪随机访问名称 | 解释                                                                                                                                                                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Uniform        | 每个文件在每个步骤中被选中的可能性相等                                                                                                                                                                              |
| Hot-and-cold   | 文件分为两组: 一组包含 10%的文件；它之所以被称为 hot，它的文件 90%都被选中。另一组被称为 cold；它包含 90%的文件，但只有 10%的时间被选中。在分组中，每个文件都有可能被选中。这种访问模式模拟了一种简单的局部性形式。 |

在这种方法中，总体磁盘容量利用率是恒定的，并且没有对读流量进行建模。模拟器运行，直到所有空白 segment 耗尽，然后模拟回收器的动作，直到空白 segment 到达阈值数从而再次可用。在每次运行中，模拟器会一直运行直到写入成本稳定、并且所有冷启动差异都已消除。

图 4 将两组模拟的结果叠加到图 3 的曲线上。在 LFS uniform 模拟中，使用了通用接入模式。回收器使用了一个简单的贪婪策略 - 总是选择利用率最低的 segment 进行清理。在 writing out live data 时，回收器没有尝试重新组织数据：live blocks 的写入顺序与它们在被清理的段中出现的顺序相同（对于通用访问模式，没有理由期望从重组织中得到任何优化）。
![图4](/static/LFS_figure4.png)
The curves labeled ‘‘FFS today’’ and ‘‘FFS improved’’ are reproduced from Figure 3 for comparison. The curve labeled ‘‘No variance’’ shows the write cost that would occur if all segments always had exactly the same utilization. The ‘‘LFS uniform’’ curve represents a log-structured ﬁle system with uniform access pattern and a greedy cleaning policy: the cleaner chooses the least-utilized segments. The ‘‘LFS hot-and-cold’’ curve represents a log-structured ﬁle system with locality of ﬁle access. It uses a greedy cleaning policy and the cleaner also sorts the live data by age before writing it out again. The x-axis is overall disk capacity utilization, which is not necessarily the same as the utilization of the segments being cleaned

即使有 uniform random access patterns，segment 利用率的变化导致大大低于从整体磁盘容量利用率和公式（1）中预测的写入成本。例如，在 75%的总体磁盘容量利用率下，被清理的段的平均利用率只有 55%。在整体磁盘容量利用率低于 20%的情况下，写入成本下降到 2.0 以下；这意味着一些被清理的段根本没有 live blocks，因此不需要被读入。

如图所述，‘‘LFS hot-and-cold’’曲线显示了访问模式中存在局部性时的写入成本。该曲线的 cleaning policy 与“LFS uniform”的相同，不同之处在于，在再次写入之前，已按 age 对 live blocks 进行了排序。这意味着冷数据倾向于在不同的段中与热数据分离；我们认为，这种方法将导致期望的的分段利用率双峰分布。

图 4 显示了一个令人惊讶的结果：与没有局部性的系统相比，局部性和“更好”的分组导致性能更差！我们尝试了改变局部性的程度（例如，95%的访问访问 5%的数据）发现性能随着局部性的增加而变得越来越差。

图 5 显示了这种反直观结果的原因。在贪婪策略下，一个段只有在成为所有段中利用率最低的一个段时才会被清理。因此，包括冷段在内，每个段的利用率最终都会下降到 cleaning threshold。不幸的是，冷段的利用率下降非常缓慢，因此这些段往往在回收点上方停留很长时间。图 5 显示，在具有局部性的模拟中，围绕回收点聚集的线段比在没有局部性的模拟中聚集的线段多得多。总体结果是，冷段往往会在很长一段时间内占用大量空闲区块。
![图5](/static/LFS_figure5.png)
These ﬁgures show distributions of segment utilizations of the disk during the simulation. The distribution is computed by measuring the utilizations of all segments on the disk at the points during the simulation when segment cleaning was initiated. The distribution shows the utilizations of the segments available to the cleaning algorithm. Each of the distributions corresponds to an overall disk capacity utilization of 75%. The ‘‘Uniform’’ curve corresponds to ‘‘LFS uniform’’ in Figure 4 and ‘‘Hot-and-cold’’ corresponds to ‘‘LFS hot-and-cold’’ in Figure 4. Locality causes the distribution to be more skewed towards the utilization at which cleaning occurs; as a result, segments are cleaned at a higher average utilization.

在研究了这些数据之后，我们意识到 cleaner 必须对热段和冷段进行不同的处理。冷段中的空闲空间比热段中的空闲空间更有价值，因为一旦冷段被回收，它将需要很长时间才能重新累积不可用的空闲空间。换句话说，一旦系统从具有冷数据的段中回收 free blocks，它将“保留”它们很长一段时间直到冷数据碎片化并再次回收它们（“takes them back again”））。相比之下，清理热段的好处不大，因为数据可能会很快消失，free space 会很快重新积累；系统还可以将清理延迟一段时间，让更多的 blocks 在当前 segment 中死亡。段空闲空间的值基于段中数据的稳定性。不幸的是，如果不知道未来的访问模式，就无法预测稳定性。假设某个数据段中的数据越老，它可能保存的时间就越长

为了测试这个理论，我们模拟了一个选择回收段的新策略。该策略根据回收段的收益和回收段的成本对每个段进行评级，并选择好处和成本比例最高的区段。收益有两个组成部分：将被回收的自由空间的数量和空间可能保持自由的时间。自由空间的数量只是 1-u，其中 u 是该段的利用率。我们使用段中任何块的最新修改时间（即最年轻的块的年龄）来估计空间可能保持自由的时间。回收的好处是由这两个部分相乘形成的时空乘积。回收区块的成本是 1+u（读取区块的一个单位成本，写回实时数据的 u）。将所有这些因素结合起来，得到：
![公式2](/static/LFS_formula2.png)
我们称之为成本-收益策略；它使冷段的回收利用率比热段高得多。

我们使用成本-收益策略和 live data 的年龄排序，在 hot-and-cold 模式下重新运行模拟。从图 6 可以看出，成本-收益策略产生了我们所希望的分段双峰分布。回收策略以大约 75%的利用率回收冷段，但在回收热段之前，要等待热段达到大约 15%的利用率。由于 90%的写入都是对热文件的，所以大部分清除的段都是热的。图 7 显示，与贪婪策略相比，成本-收益策略将写入成本降低了多达 50%，而且 LFS 即使在磁盘容量利用率相对较高的情况下，也比 Unix FFS 表现得更好。我们模拟了许多其他程度和类型的局部性，发现随着局部性的增加，成本-收益策略变得更好。
![图6](/static/LFS_figure6.png)
This figure shows the distribution of segment utilizations from the simulation of a hot-and-cold access pattern with 75%overall disk capacity utilization.The "LFS Cost-Benefit''curve shows the segment distribution occurring when the cost-benefit policy is used to select segments to clean and live blocks grouped by age before being re-written.Because of this bimodal segment distribution,most of the segments cleaned had utilizations around 15%. For comparison,the distribution produced by the greedy method selection policy is shown by the 'LFS Greedy''curve reproduced from Figure 5.

模拟实验说服我们在 Sprite LFS 中实现成本-收益策略。将会在 5.2 节看到，使用 Sprite LFS 的实际文件系统的行为甚至比图 7 预测更好。
![图7](/static/LFS_figure7.png)
This graph compares the write cost of the greedy policy with that of the cost-benefit policy for the hot-and-cold access pattern.The cost-benefit policy is substantially better than the greedy policy, particularly for disk capacity utilizations above 60%.

## 3.6. Segment usage table
为了支持成本-收益策略，Sprite LFS 维护了一个称为 segment usage table 的数据结构。对于每个段，该表记录段中的 live bytes 以及段中任何 block 的最近修改时间。选择要清理的 segments 时，回收器将使用这两个值。这些值最初是在写入段时设置的，而在删除文件或覆盖块时， live bytes 的计数会减少。如果计数降至零，则可以在不清洗的情况下重复使用该段。segment usage table 的 blocks 被写入 log，而那些 blocks 的地址存储在 checkpoint中（详见第 4 节）。

为了按年龄对 live blocks 进行排序，segment summary information(段摘要信息？)记录写入段中的最年轻的 block 的时间。目前，Sprite LFS 没有记录 file 中每个 block 的修改时间；它为整个 file 只记录一个修改时间。对于未全部修改的文件，此记录时间是不正确的。我们计划修改 segment summary information（段摘要信息），以记录每个 block 的修改时间。

# 4. Crash recovery
当系统崩溃时，在磁盘上执行的最后几次操作可能会存在不一致的状态（例如，可能写入了一个新文件，但没有写入包含该文件的目录）；在重启时，操作系统必须检查这些操作，以纠正任何不一致。在没有 log 的传统 Unix 文件系统中，系统无法确定上次更改的位置，因此必须扫描磁盘上的所有 metadata 以恢复一致性。这些扫描的成本已经很高（在常规配置中需要几十分钟），而且随着存储系统的扩展，成本也越来越高。

在 LFS 中，最后一次磁盘操作的位置很容易确定：它们位于 log 的末尾。因此，奔溃后应该可以很快恢复。logs 的这一优点是众所周知的，并且在数据库系统和其他文件系统中都得到了利用。与许多其他 logging system 一样，Sprite LFS 使用双管齐下的恢复方法：checkpoints（定义文件系统的一致状态）和 roll-forward（recover information written since the last checkpoint）

## 4.1. Checkpoints
Checkpoint is a position in the log at which all of the ﬁle system structures are consistent and complete。

Sprite LFS 使用一个 two-phase process 来创建检查点:
1. 首先，它将所有修改的信息写入 log，包括 file data blocks、indirect blocks、inodes 以及 inode map 和 segment usage table
2. 其次，它将 checkpoint region 写入磁盘上的一个特殊的固定位置。checkpoint region contains the addresses of all the blocks in the inode map and segment usage table, plus the current time and a pointer to the last segment written.

在重启时，Sprite LFS 读取 checkpoint region，并使用该信息初始化其主内存数据结构。为了处理 checkpoint 操作期间崩溃的问题，实际上有两个 checkpoint region，checkpoint operations alternate between them。checkpoint time 在 checkpoint area 的最后一个 block 中，因此如果检查点失败，时间将不会更新。在重新启动期间，系统读取两个 checkpoint area，并使用拥有最新时间的 checkpoint area。

Sprite LFS 周期写入 checkpoints，以及在卸载文件系统或关机时也会写入 checkpoints。检查点之间的长时间间隔减少了写入检查点的开销，但增加了恢复期间 roll forward 所需的时间；较短的检查点间隔可以缩短恢复时间，但会增加正常操作的成本。Sprite LFS 目前使用 30 秒的检查点间隔，这可能太短了。定期检查点的另一种选择是在给定数量的新数据写入 log 后写入检查点；这样即能保证奔溃恢复不会太久、也减少了执行写入 checkpoints 的开销。

## 4.2. Roll-forward
原则上，只要读取最新的 checkpoint region 并丢弃在该检查点之后 log 中的所有数据，就可以瞬间恢复，但是哪个 checkpoint 之后写的数据就丢失了。为了恢复尽可能多的信息，Sprite LFS 扫描在最后一个 checkpoint 之后写入的 log segments，此操作称为 roll-forward。

在 roll-forward 期间，Sprite LFS 使用 segment summary blocks 中的信息来恢复最近写入的 file data:
1. 当 summary block 指示存在新的 inode 时，Sprite LFS 会更新它从 checkpoint 读取的 inode map，以便 inode map 引用 inode 的新副本。这会自动将文件的新数据块合并到恢复的文件系统中。
2. 如果在没有文件 inode 新副本的情况下发现文件的新 data block，则假定磁盘上文件的新版本不完整，并忽略发现的新 data block。

 roll-forward 代码还调整从检查点读取的 segment usage table 的利用率。检查点之后写入的段的利用率将为零（？？这个意思是抛弃检查点之后写入的段并将其认为为 clean segment？）；它们必须调整为参考‌选择 roll-forward 后留下的实时数据。旧段的利用率也必须调整为能够反应出 file 的删除和覆盖（这两个都可以通过 log 中是否存在新的 inode 来识别）。

 roll-forward 的最后一个问题是如何恢复 directory entries 和 inodes 之间的一致性。每个 inode 都包含一个指向该 inode 的 directory entries 的计数；当计数降至零时，文件被删除。不幸的是，崩溃可能发生在拥有新的 reference count 的 inode 信息已经写入 log 但是对应的 directory entry 还没有写入，反之亦然。

为了恢复 directory 和 inode 之间的一致性，Sprite LFS 在 log 中为每个目录更改输出一条特殊记录（special record）。记录包括 operation code（create, link, rename, or unlink）、directory entry 的位置（(i-number for the directory and the position within the directory）、the contents of the directory entry (name and i-number), 以及 new reference count for the inode named in the entry。这些记录统称为 directory operation log；Sprite LFS 保证每个 directory operation log entry 出现在 log 早于相应的 directory block 或 inode。

在 roll-forward, directory operation log 用于确保 directory entries 和 inode 之间的一致性：如果出现 log entry，但 inode 和 directory block 未同时写入，则 roll-forward 会更新 directory 和/或 inode 完成操作。Roll-forward operations can cause entries to be added to or removed from directories and reference counts on inodes to be updated。The recovery program appends the changed directories, inodes, inode map, and segment usage table blocks to the log and writes a new checkpoint region to include them. The only operation that can’t be completed is the creation of a new ﬁle for which the inode is never written; in this case the directory entry will be removed. In addition to its other functions, the directory log made it easy to provide an atomic rename operation.

The interaction between the directory operation log and checkpoints introduced additional synchronization issues into Sprite LFS. In particular, each checkpoint must represent a state where the directory operation log is consistent with the inode and directory blocks in the log. This required additional synchronization to prevent directory modiﬁcations while checkpoints are being written.

# 5. Experience with the Sprite LFS
1989 年底，我们开始实现 Sprite LFS，到 1990 年年中，它已作为 Sprite 网络操作系统的一部分投入运行。自 1990 年秋天以来，它已被用于管理五个不同的磁盘分区，大约有 30 个用户使用这些分区进行日常计算。本文中描述的所有功能都已在 Sprite LFS 中实现，但尚未在生产环境中使用 roll-forward 功能。生产磁盘使用较短的 checkpoint interval（30 秒），重启时丢弃最后一个检查点之后的所有信息。

当我们开始这个项目时，我们担心 LFS 可能比传统的文件系统要复杂得多。然而，实际上，Sprite LFS 并不比 Unix FFS 复杂：Sprite LFS 额外增加了实现 segment cleaner 的复杂性，但这可以通过消除 Unix FFS 所需的 bitmap 和 layout policies 补偿；此外，Sprite LFS 中的 checkpoint 和 roll-forward 代码并不比扫描 Unix FFS 磁盘以恢复一致性的 fsck 代码复杂。与 Unix FFS 或 Sprite LFS 相比，像 Episode 或 Cedar 这样的 Logging file system 可能要复杂一些，因为它们同时包含 logging 和 layout code。

在日常使用中，与类似 Unix FFS 的文件系统相比，用户对于 Sprite LFS 并没有感觉什么不同。原因是正在使用的机器的负载下、还达不到 disk 操作成为瓶颈的程度。例如，在修改后的 Andrew 基准测试中，Sprite LFS 比使用第 5.1 节中介绍的配置的 SunOS 只快 20%。大部分加速都是由于在 Sprite LFS 中删除了同步写入带来的。即使使用 Unix FFS 的同步写入，基准测试的 CPU 利用率也超过了 80%，限制了磁盘存储管理的变化可能带来的加速。

## 5.1. Micro-benchmarks
我们使用了一系列小型基准测试来衡量 Sprite LFS 的最佳性能，并将其与 SunOS 4.0.3(其文件系统基于 Unix FFS) 进行比较。基准测试是虚构的，因此它们不代表实际的工作负载，但它们说明了这两个文件系统的优缺点。用于这两个系统的机器是一台 Sun-4/260（8.7 integer SPECmarks），内存为 32 兆字节，一台 Sun SCSI3 HBA 和一个 Wren IV 磁盘（最大传输带宽为 1.3 兆字节/秒，平均寻道时间为 17.5 毫秒）。对于 LFS 和 SunOS，磁盘都是用一个文件系统格式化的，该文件系统有大约 300 兆的可用存储空间。SunOS 使用 8KB 的 block 大小，而 Sprite LFS 使用 4KB 的 block 大小和 1MB 的 segment 大小。在每种情况下，系统都运行多用户，但在测试过程中处于静止状态。对于 Sprite LFS，在基准测试运行期间没有进行回收，因此测量结果代表了最佳情况下的性能；有关回收开销的测量，请参见下文第 5.2 节。

图 8 显示了创建、读取和删除大量小文件的基准测试的结果。在基准测试的创建和删除阶段，Sprite LFS 的速度几乎是 SunOS 的十倍。Sprite LFS 读取文件的速度也更快；这是因为文件的读取顺序与创建的顺序相同，LFS 将文件密集地打包在 log 中。此外，在创建阶段，Sprite LFS 仅使磁盘保持 17%的繁忙状态，同时使 CPU 饱和。相比之下，SunOS 在创建阶段让磁盘 85%的时间处于繁忙状态，尽管只有大约 1.2%的磁盘潜在带宽用于新数据。这意味着，随着 CPU 速度的提高，Sprite LFS 的性能将提高 4-6 倍（见图 8（b））、而 SunOS 几乎不会有任何提高。
![图8](/static/LFS_figure8.png)
Figure (a) measures a benchmark that created 10000 one-kilobyte ﬁles, then read them back in the same order as created, then deleted them. Speed is measured by the number of ﬁles per second for each operation on the two ﬁle systems. The logging approach in Sprite LFS provides an order-of-magnitude speedup for creation and deletion. Figure (b) estimates the performance of each system for creating ﬁles on faster computers with the same disk. In SunOS the disk was 85% saturated in (a), so faster processors will not improve performance much. In Sprite LFS the disk was only 17% saturated in (a) while the CPU was 100% utilized; as a consequence I/O performance will scale with CPU speed.

尽管 Sprite 的设计目的是提高在多小文件访问工作负载下的效率，但图 9 显示，大文件环境下提供了具有竞争力的性能。Sprite LFS 在所有情况下都比 SunOS 具有更高的写入带宽。随机写入的速度要快得多，因为它将随机写入转换为对 log 的顺序写入；对于顺序写入，它的速度也更快，因为它将许多 blocks 分组到单次大型 I/O 中，而 SunOS 对每个 block 执行单独的磁盘操作（更新版本的 SunOS 执行分组批量写入，因此其性能应与 Sprite LFS 相当）。这两个系统在 file 随机写入后顺序读的场景下读性能相似，在这种情况下，Sprite LFS 要进行寻道，因此其性能大大低于 SunOS。

图 9 说明 LFS 在磁盘上产生的局部性与传统文件系统不同。传统的文件系统通过假设某些访问模式（文件的顺序读取、在一个目录中使用多个文件的趋势等）来实现逻辑上的局部性，其会有额外的写成本 - 以便根据假定的读取模式在磁盘上以最佳方式组织信息。与之相反，LFS 实现的是时间局部性：同时创建或修改的信息将在磁盘上紧密分组。如果时间局部性与逻辑局部性相匹配，就像顺序写入然后顺序读取的文件一样，那么 LFS 在大文件上的性能应该与传统文件系统大致相同。如果时间局部性不同于逻辑局部性，那么系统的性能将不同。**Sprite LFS 处理随机写入的效率更高，因为它会在磁盘上按顺序写入**。为了实现逻辑局部性，SunOS 的随机写入代价更大，但之后它可以更有效地处理顺序重读。在这两个系统中，随机读取的性能大致相同，**尽管 blcok 的布局非常不同**。然而，如果非顺序读取的顺序与非顺序写入的顺序相同，那么 Sprite LFS 会快得多。

## 5.2. Cleaning overheads
上一节的微基准测试结果对 Sprite LFS 的性能给出了乐观的看法，因为它们不包括任何回收开销（基准测试运行期间的写入成本为 1.0）。为了评估回收成本和 cost-beneﬁt cleaning policy 的有效性，我们记录了几个月内生产 LFS 的统计数据。测量了五个 system:

| system      | meaning                                                                                                                                                                                   |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| /user6      | Sprite 开发者的主目录。工作量包括程序开发、文本处理、电子通信和模拟                                                                                                                       |
| /pcs        | Home directories and project area for research on parallel processing and VLSI circuit design.                                                                                            |
| /src/kernel | Sources and binaries for the Sprite kernel.                                                                                                                                               |
| /swap2      | Sprite client workstation swap ﬁles. Workload consists of virtual memory backing store for 40 diskless Sprite workstations. Files tend to be large, sparse, and accessed nonsequentially. |
| /tmp        | Temporary ﬁle storage area for 40 Sprite works-tations.                                                                                                                                                                                          |

表 2 显示了四个月回收期间收集的统计数据。为了消除启动影响，我们在文件系统投入使用后等待了几个月才开始测量。LFS 的表现大大好于第 3 节中的模拟预测。尽管整体磁盘容量利用率在 11-75%之间，但清理的磁盘段中有一半以上是空的。即使是非空段的利用率也远低于平均磁盘利用率。与相应模拟中 2.5-3 的写入成本相比，总体写入成本在 1.2 到 1.6 之间。图 10 显示了在/user6 磁盘的最新快照中收集的段利用率的分布。
![表2](/static/LFS_table2.png)

For each Sprite LFS ﬁle system the table lists the disk size, the average ﬁle size, the average daily write trafﬁc rate, the average disk capacity utilization, the total number of segments cleaned over a four-month period, the fraction of the segments that were empty when cleaned, the average utilization of the non-empty segments that were cleaned, and the overall write cost for the period of the measurements. These write cost ﬁgures imply that the cleaning overhead limits the long-term write performance to about 70% of the maximum sequential write bandwidth.

![图10](/static/LFS_figure10.png)
This figure shows the distribution of segment utilizations in a recent snapshot of the /user6 disk.The distribution shows large numbers of fully utilized segments and totally empty segments.

我们认为，有两个原因可以解释为什么 Sprite LFS 的回收成本低于模拟:
1. 首先，模拟中的所有文件都只有一个块长。实际上，有大量较长的文件，它们往往作为一个整体写入和删除。这会导致在各个片段中产生更大的局部性。在最好的情况下，如果一个文件比一个段长得多，删除该文件将产生一个或多个完全空的段。模拟和现实之间的
2. 第二个区别是，模拟的参考模式均匀分布在热文件组和冷文件组中。实际上，有大量的文件几乎从未写入（实际上，冷段比模拟中的冷段冷得多）。LFS会将非常冷的文件分段隔离，并且永远不会清理它们。在模拟中，每个片段最终都会被修改，因此必须被清理。

如果第 5.1 节中的 Sprite LFS 测量值有点过于乐观，那么本节中的测量值就过于悲观了。在实践中，可能会在夜间或其他空闲时间执行大部分回收工作，以便在突发活动期间可以使用回收段。我们还没有足够的经验直到在 Sprite LFS 中这是否可以做到。此外，我们希望随着经验的积累和算法的调整，Sprite LFS 的性能会有所提高。例如，我们尚未仔细分析一次清理多少段的策略问题，但我们认为这可能会影响系统将热数据与冷数据分离的能力。

## 5.3. Crash recovery
尽管崩溃恢复代码并未上线到生产系统，the code works well enough to time recovery of various crash scenarios(？？这里 time 作为动词的意思是什么？)。恢复时间取决于检查点间隔以及执行的操作的速率和类型。表 3 显示了不同文件大小和数据量的恢复时间。不同的崩溃配置是通过运行一个程序生成的，该程序在系统崩溃前创建 1、10 或 15 兆字节的固定大小文件。这里使用了一个特定版本的 Sprite LFS（它有一个无限的检查点间隔，从不将目录更改写入磁盘）。在 roll-forward 恢复期间，必须将创建了的文件们添加到 inode map 中，创建 directory entries，并更新 segment usage table。

表 3 显示，恢复时间随最后一个检查点和崩溃之间写入的文件的数量和大小而变化。通过限制检查点之间写入的数据量，可以限制恢复时间。从表 2 中的平均文件大小和每日写入流量来看，1 小时的检查点间隔的平均恢复时间约为 1 秒。使用观测到的最最大 150 兆字节/小时的写入速率，检查点间隔长度每增加 70 秒，最大恢复时间将增加 1 秒。
![表3](/static/LFS_table3.png)
The table shows the speed of recovery of one,ten,and fifty megabytes of fixed-size files.The system measured was the same one used in Section 5.1.Recovery time is dominated by the number of files to be recovered.

## 5.4. Other overheads in Sprite LFS
表 4 显示了写入磁盘的各种数据的 relative importance，包括它们在磁盘上占据了多少 live blocks，以及它们写入 log 的数据量。磁盘上 99%以上的 live data 由 file data blocks 和 indirect blocks 组成。然而，写入 log 的信息中约有 13%由 inode、inode map blocks 和 segment map blocks 组成，所有这些信息都会很快被覆盖。仅 inode map 就占写入 log 所有数据的 7%以上。我们怀疑这是因为 Sprite LFS 中当前使用的检查点间隔较短，这迫使 metadata 不必要的频繁存储到磁盘。如果上线 roll-forward recovery 代码并增大检查点间隔，我们预计 metadata 的 log 带宽开销将大幅下降。
![表4](/static/LFS_table4.png)
For each block type,the table lists the percentage of the disk space in use on disk (Live data)and the percentage of the log bandwidth consumed writing this block type(Log bandwidth). The block types marked with '\*' have equivalent data structures in Unix FFS.

# 6. Related work
LFS 概念和 Sprite LFS 设计借鉴了许多不同存储管理系统的思想。具有类似日志结构的文件系统已经出现在一些关于在一次写入介质上构建文件系统的提案中。除了仅以附加的方式写入所有更改外，这些系统还维护索引信息(就像 Sprite LFS 的 inode map 和 inodes 一样)，用于快速定位和读取文件。它们与 Sprite LFS 的不同之处在于，介质的一次写入特性使得文件系统无需回收 log space。

Sprite LFS 中使用的段清理回收方法非常类似于为编程语言开发的垃圾回收器。在 Sprite LFS 中清理段的过程中，cost-beneﬁt 段选择和块的年龄排序将文件分为几部分，就像 generational garbage collection schemes。这些垃圾回收方案与 Sprite LFS 之间的一个显著区别是，在 generational garbage collectors 中可以进行高效的随机访问，而在文件系统中实现高性能需要顺序访问。此外，Sprite LFS 可以利用 block 一次最多只能属于一个文件这一特征，比编程语言系统中使用的更简单的算法来识别垃圾。

Sprite LFS 中使用的 logging scheme 类似于数据库系统中开创的方案。几乎所有的数据库系统都使用 WAL 来实现崩溃恢复和高性能，但在使用 log 的方式上与 Sprite LFS 有所不同。Sprite LFS 和数据库系统都将 log 视为磁盘上数据状态的最新“真相”。主要区别在于数据库系统不使用 log 作为数据的最终存储：其为此保留了一个单独的数据区域。这些独立数据区域意味着它们不需要 Sprite LFS 的段清理机制来回收 log space。当记录的更改被写入其最终位置时，可以回收数据库系统中 log 占用的空间。由于所有读请求都是从数据区处理的，因此可以在不影响读取性能的情况下压缩日志。通常，只有更改的字节才会写入数据库 log，而不是像 Sprite LFS 中那样写入全部块。

使用“redo log”的检查点和 roll forward 的 Sprite LFS 崩溃恢复机制类似于数据库系统和对象存储中使用的技术。Sprite LFS 中的实现更简单，因为 log 是数据的最终归宿。Sprite LFS recovery 确保索引指向日志中数据的最新副本，而不是 redoing the operation to the separate data copy。

在 file cache 中缓存数据并将其大量的一次性写入磁盘类似于数据库系统中的 group commit 概念，也类似于主存数据库系统中使用的技术

# 7. Conclusion
LFS 背后的基本原理很简单：在主内存中的 file cache 中收集大量新数据，然后用一个可以占满磁盘所有带宽的大型 I/O 将数据写入磁盘。实现这个想法的复杂度在于需要在磁盘上保持较大的空闲区域，但我们的模拟分析和 Sprite LFS 的经验都表明，基于成本-效益的简单策略可以实现较低的清洗开销。虽然我们开发了一个 LFS 系统来支持多小文件特征的工作负载，但这种方法也适用于大文件访问。特别的，对于创建和删除的非常大的文件，基本没有任何清理开销。

归根结底，LFS 可以比现有的文件系统更高效地使用磁盘。在 I/O 限制再次成为计算机系统的可扩展性的瓶颈之前，LFS 将使我们能够更好的利用最近几代更快的处理器。

[^a]: read back files 是什么意思？
[^2]: inode (index node)是指在许多“类 Unix 文件系统”中的一种数据结构，用于描述文件系统对象（包括文件、目录、设备文件、socket、管道等）。每个 inode 保存了文件系统对象数据的属性和磁盘块位置。文件系统对象属性包含了各种元数据（如：最后修改时间） ，也包含用户组（owner ）和权限数据。
[^3]: 指一段连续的存储空间。一般来说，一个文件的物理大小一定是一个 extent 容量的整数倍。当一个进程创建一个文件的时候，文件系统管理程序会将整个 extent 分配给这个文件。当再次向该文件写入数据时 (有可能是在其他写入操作之后)，数据会从上次写入的数据末尾处追加数据。


#todo 将回收相关概念替换为回收
