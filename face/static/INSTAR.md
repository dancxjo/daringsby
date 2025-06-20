## Pete d'Isle-Ashe: Features and Vision

Each iteration of the **Pseudo-conscious Experiment in Thoughts & Emotion** (PETE) builds upon the lessons of its predecessors, evolving into a more refined system. This instar, named Pete d'Isle-Ashe, continues the tradition of adopting a pompous last name while introducing significant improvements over the venerable Pete Humfreeze. As a "butterfly," Pete undergoes progressive transformations, each iteration honing its features to new heights.

### Core Improvements in Pete d'Isle-Ashe

1. **Stop Reinventing the Wheel**

   - Transition to **LangChain.js** for function calling capabilities. LangChain.js offers robust tools for managing function execution, vastly improving on previous custom implementations.

2. **Streamlined Execution**

   - Embrace **streaming** wherever possible. Function calls will execute as soon as arguments begin arriving, reducing latency by streaming data progressively. This ensures prompt and efficient processing.

3. **Sentences as Units of Thought**

   - Nodes in Pete's graph database are based on sentences, as embeddings operate most effectively at this level. Key features of this schema include:
     - **Content Storage**: Sentences are stored along with their vector representations. For specialized nodes (e.g., `:Self`), a meaningful description is used for vectorization.
     - **Hierarchical Links**:
       - `:FOLLOWS` links establish sequence relationships between sentences.
       - `:SUMMARIZES` links connect sentences to higher-level summaries, enabling hierarchical navigation.
     - **Focused Retrieval**: By navigating the hierarchy, Pete can narrow the vector space and retrieve contextually relevant nodes, avoiding information overload. Nearest-neighbor search can be complemented by pathfinding between nodes.

4. **Thought Tagging with Hashtags**

   - Pete uses hashtags to index and prioritize thoughts. Prompts to the LLM extract important tags for each thought, ensuring efficient storage and retrieval.

5. **Task Management with Limited LLMs**

   - Carry over a structured "wit cycle" based on **prime-number intervals** to schedule prompts:
     - **Every Tick**: Linguistic Processor A generates the next thought. The response is parsed into vectorized nodes for the graph, activating recall processes and enriching the conversation buffer. This can be done asynchronously alongside other tasks handled by Linguistic Processor B.
     - **Every Other Tick**: Visual and sensory input (e.g., camera frames) is processed into a "twinkling" of sensations, bundled chronologically.
     - **Every Third Tick**: Linguistic Processor B synthesizes twinklings into "instants," creating higher-order representations.
     - **Every 17th Tick**: Instants are synthesized into "moments," contributing to the overall context.
     - **Context Transformation**: Moments form the basis for generating Cypher queries, transforming high-level context into actionable insights.

6. **Multi-Processor Coordination**

   - Leverage asynchronous coordination between Linguistic Processors A and B. Processor A handles immediate thought generation and recall, while Processor B focuses on synthesis and abstraction.

7. **Entity Identification and Management**

   - Carry over processes within the wit cycle to identify important entities in the graph, assign them meaningful descriptions, and merge overlapping entities. Key steps include:
     - **Identification**: Use metrics such as frequency of access or graph centrality to flag high-priority nodes.
     - **Description**: Generate or refine descriptions using LLMs to summarize linked nodes and properties.
     - **Merging**: Consolidate similar entities by combining relationships and synthesizing unified descriptions, ensuring a cleaner, more efficient graph.

### Summary of Processes

- **Streaming and Context**: By implementing streaming and hierarchical sentence nodes, Pete can efficiently narrow the focus within the vector space, improving the speed and relevance of retrieval-augmented generation (RAG).
- **Hierarchical Organization**: The combination of `:FOLLOWS` and `:SUMMARIZES` relationships allows for scalable, Yahoo-style navigation of Pete's thoughts.
- **Prime-Based Task Cycling**: Progressive abstraction and synthesis are managed with a structured wit cycle, balancing immediate and long-term processing needs.
- **Enhanced Retrieval and Interaction**: Tagging and contextual synthesis enable Pete to operate dynamically, providing nuanced insights while maintaining system efficiency.
- **Entity Management**: Identification, description, and merging of important entities ensure the graph remains clean, optimized, and semantically rich.

Pete d'Isle-Ashe represents a leap forward in the PETE project, combining advanced techniques with lessons learned from prior iterations. By grounding thoughts in sentences, leveraging streaming, and employing structured task management, this iteration aims to deliver on the promise of a truly pseudo-conscious system.

