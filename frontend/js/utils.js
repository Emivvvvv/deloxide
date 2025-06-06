/**
 * Deloxide - Deadlock Detection Visualization Utilities
 *
 * This file contains utility functions for processing and transforming
 * raw deadlock log data into a format usable by the visualization.
 */

/**
 * Convert raw lock number to string representation
 * Returns the original lock number as a string
 */
function mapLockId(lockNum) {
  return lockNum.toString()
}

/**
 * Transform raw logs into structured log entries
 *
 * @param {Array} rawLogs - Each element is [raw_thread_id, raw_lock, event_code, timestamp, parent_id]
 * @param {Object} resourceMapping - { raw_lock: letter } e.g. {1: "A", 2: "B"}
 *
 * In logs, thread_id is kept as the original raw value, and timestamp is
 * converted to milliseconds (unix epoch ms) if the raw data is in seconds.
 */
function transformLogs(rawLogs, resourceMapping) {
  // We will generate log entries incrementally rather than showing all threads and resources at the start
  // Create a mapping for thread IDs
  const threadMapping = {}
  let nextThreadIdx = 1;
  
  // Find the first parent_id that's not 0 for marking as main thread
  let mainThreadId = null;
  for (const log of rawLogs) {
    const [rawThread, lockNum, eventCode, timestamp, parentId] = log;
    if (parentId !== 0 && mainThreadId === null) {
      mainThreadId = parentId;
      break;
    }
  }
  
  console.log("Identified main thread ID:", mainThreadId);
    
  // Normal logs: Each thread_id is kept as the original raw value
  // The timestamp value is converted to milliseconds if the raw data is in seconds
  // We start with empty log array and build it up
  const logs = [];
  
  // First entry is an empty graph
  const initLog = {
    step: 1,
    timestamp: null, // Set to null to prevent showing timestamp for step 1
    type: "init",
    description: "Waiting for threads and resources to arrive...<br>Sit back, relax, and enjoy the calm before the deadlocks.",
  };
  
  logs.push(initLog);
  
  // Event type mapping including new spawn and exit events
  const eventTypes = { 0: "attempt", 1: "acquired", 2: "released", 3: "spawn", 4: "exit" };
  
  // Keep track of resource ownership and waiting threads for deadlock detection
  const resourceOwners = {}; // Maps resource_id to thread_id that owns it
  const threadWaiting = {}; // Maps thread_id to resource_id it's waiting for
  
  // Flag to track if a deadlock has been detected
  let deadlockDetected = false;
  
  // Process each log entry
  for (let idx = 0; idx < rawLogs.length; idx++) {
    // If deadlock already detected, stop processing more logs
    if (deadlockDetected) break;
    
    const log = rawLogs[idx];
    const [rawThread, lockNum, eventCode, timestamp, parentId] = log;
    
    // Assign thread mapping if not already assigned
    if (!(rawThread in threadMapping) && rawThread !== 0) {
      threadMapping[rawThread] = nextThreadIdx++;
    }
    
    const type = eventTypes[eventCode] || "unknown";
    
    // Skip unknown event types
    if (type === "unknown") continue;
    
    // Update resource ownership tracking for deadlock detection
    if (type === "acquired" && rawThread !== 0 && lockNum !== 0) {
      resourceOwners[lockNum] = rawThread;
      // Thread is no longer waiting
      delete threadWaiting[rawThread];
    }
    else if (type === "attempt" && rawThread !== 0 && lockNum !== 0) {
      // Thread is now waiting for this resource
      threadWaiting[rawThread] = lockNum;
      
      // Check for deadlock after every attempt event
      const deadlockCycle = detectDeadlockCycle(resourceOwners, threadWaiting);
      if (deadlockCycle && deadlockCycle.length >= 2) {
        deadlockDetected = true;
      }
    }
    else if (type === "released" && rawThread !== 0 && lockNum !== 0) {
      // Resource is no longer owned by this thread
      if (resourceOwners[lockNum] === rawThread) {
        delete resourceOwners[lockNum];
      }
    }
    
    // For spawn events: Either a thread is spawned or a resource is created
    if (type === "spawn") {
      let description = "";
      
      if (rawThread !== 0) {
        // Thread spawn - parentId is the thread that spawned it
        let parentName;
        if (parentId === 0) {
          parentName = "main thread";
        } else if (parentId === mainThreadId) {
          parentName = `<span class="main-thread">Main Thread</span>`;
        } else {
          parentName = `<span class="thread-id">Thread ${parentId}</span>`;
        }
        
        description = `${parentName} spawned <span class="thread-id">Thread ${rawThread}</span>.`;
      } else if (lockNum !== 0) {
        // Resource creation
        let parentName;
        if (parentId === 0) {
          parentName = "main thread";
        } else if (parentId === mainThreadId) {
          parentName = `<span class="main-thread">Main Thread</span>`;
        } else {
          parentName = `<span class="thread-id">Thread ${parentId}</span>`;
        }
        
        description = `<span class="resource-id">Resource ${resourceMapping[lockNum]}</span> generated by ${parentName}.`;
      }
      
      logs.push({
      step: idx + 2, // init step is 1, so we start from 2
      timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
        type,
        thread_id: rawThread,
        resource_id: lockNum !== 0 ? resourceMapping[lockNum] : null,
        parent_id: parentId,
        description,
        is_main_thread: rawThread === mainThreadId, // Mark if this is the main thread
      });
    } 
    // For exit events: Either a thread exits or a resource is dropped
    else if (type === "exit") {
      let description = "";
      
      if (rawThread !== 0) {
        // Thread exit
        if (rawThread === mainThreadId) {
          description = `<span class="main-thread">Main Thread</span> exited.`;
        } else {
          description = `<span class="thread-id">Thread ${rawThread}</span> exited.`;
        }
        
        // Clean up tracking for this thread
        delete threadWaiting[rawThread];
        // Remove from resource owners
        Object.keys(resourceOwners).forEach(res => {
          if (resourceOwners[res] === rawThread) {
            delete resourceOwners[res];
          }
        });
      } else if (lockNum !== 0) {
        // Resource drop
        description = `<span class="resource-id">Resource ${resourceMapping[lockNum]}</span> dropped.`;
        
        // Clean up tracking for this resource
        delete resourceOwners[lockNum];
      }
      
      logs.push({
        step: idx + 2, // init step is 1, so we start from 2
        timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
        type,
        thread_id: rawThread,
        resource_id: lockNum !== 0 ? resourceMapping[lockNum] : null,
        description,
        is_main_thread: rawThread === mainThreadId, // Mark if this is the main thread
      });
    }
    // For normal resource events
    else {
      let threadDescription;
      if (rawThread === mainThreadId) {
        threadDescription = `<span class="main-thread">Main Thread</span>`;
      } else {
        threadDescription = `<span class="thread-id">Thread ${rawThread}</span>`;
      }
      
      // Update description based on type
      let actionText = type;
      if (type === "attempt") {
        actionText = "attempted to acquire";
      }
      
      logs.push({
        step: idx + 2, // init step is 1, so we start from 2
        timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
        type,
        thread_id: rawThread,
        resource_id: resourceMapping[lockNum],
        description: `${threadDescription} ${actionText} <span class="resource-id">Resource ${resourceMapping[lockNum]}</span>`,
        is_main_thread: rawThread === mainThreadId, // Mark if this is the main thread
      });
    }
  }

  // Detect deadlock using the resource ownership and waiting threads information
  const deadlockCycle = detectDeadlockCycle(resourceOwners, threadWaiting);
  
  if (deadlockCycle && deadlockCycle.length >= 2) {
    // Create descriptions of the deadlock cycle
    const deadlockDescriptions = [];
    
    for (let i = 0; i < deadlockCycle.length; i++) {
      const threadId = deadlockCycle[i];
      const waitingForResource = threadWaiting[threadId];
      const resourceHeldByThread = resourceOwners[waitingForResource];
      const resourceSymbol = resourceMapping[waitingForResource];
      
      // Format thread display based on whether it's the main thread
      const threadDisplay = threadId === mainThreadId 
        ? `<span class="main-thread">Main Thread</span>` 
        : `<span class="thread-id">Thread ${threadId}</span>`;
      
      // Format resource owner display based on whether it's the main thread
      const resourceOwnerDisplay = resourceHeldByThread === mainThreadId
        ? `<span class="main-thread">Main Thread</span>`
        : `<span class="thread-id">Thread ${resourceHeldByThread}</span>`;
      
      deadlockDescriptions.push(`${threadDisplay} is waiting for <span class="resource-id">Resource ${resourceSymbol}</span> held by ${resourceOwnerDisplay}`);
    }

    // Join descriptions with proper separators for better readability
    let deadlockDescription = `<strong>DEADLOCK DETECTED:</strong><br>`;
    deadlockDescription += deadlockDescriptions.join('<br>');

    // Last log timestamp: 100 ms after the last event (in milliseconds)
    const lastTimestamp = logs.length > 1 
      ? logs[logs.length - 1].timestamp 
      : Math.floor(Date.now());
    const deadlockTimestamp = lastTimestamp + 100;
      
    const deadlockLog = {
      step: logs.length + 1,
      timestamp: deadlockTimestamp,
      type: "deadlock",
      cycle: deadlockCycle,
      description: deadlockDescription,
      deadlock_details: {
        thread_cycle: deadlockCycle,
        thread_waiting_for_locks: Object.entries(threadWaiting)
          .filter(([threadId]) => deadlockCycle.includes(parseInt(threadId)) || deadlockCycle.includes(threadId))
          .map(([threadId, resourceId]) => ({
            thread_id: parseInt(threadId),
            lock_id: resourceMapping[resourceId],
            resource_id: resourceId
          })),
        timestamp: deadlockTimestamp,
      },
    };
      
    logs.push(deadlockLog);
    
    console.log("Deadlock detected, showing only events up to deadlock");
  }

  return logs;
}

/**
 * Detects a deadlock cycle in the system
 * 
 * @param {Object} resourceOwners - Maps resource_id to thread_id that owns it
 * @param {Object} threadWaiting - Maps thread_id to resource_id it's waiting for
 * @returns {Array|null} - Array of thread IDs in the deadlock cycle, or null if no deadlock
 */
function detectDeadlockCycle(resourceOwners, threadWaiting) {
  // If no threads are waiting, there can't be a deadlock
  if (Object.keys(threadWaiting).length === 0) {
    return null;
  }
  
  // Build a wait-for graph
  // Each key is a thread, value is a thread that it's waiting for
  const waitForGraph = {};
  
  Object.entries(threadWaiting).forEach(([waitingThreadId, resourceId]) => {
    // Convert to numbers for consistent comparison
    const waitingThread = parseInt(waitingThreadId);
    const resourceOwner = resourceOwners[resourceId];
    
    // If the resource has an owner, add an edge from waiting thread to owner
    if (resourceOwner !== undefined) {
      waitForGraph[waitingThread] = resourceOwner;
    }
  });
  
  // No wait-for graph edges means no deadlock
  if (Object.keys(waitForGraph).length === 0) {
    return null;
  }
  
  // Detect cycles using DFS (Depth-First Search)
  const visited = {};
  const recStack = {};
  let cycle = null;
  
  function detectCycle(node, path = []) {
    // Mark the current node as visited and add to recursion stack
    visited[node] = true;
    recStack[node] = true;
    path.push(node);
    
    // Check if this node has any neighbors (is waiting for any thread)
    const neighbor = waitForGraph[node];
    if (neighbor !== undefined) {
      // If the neighbor is in the recursion stack, we found a cycle
      if (recStack[neighbor]) {
        // Extract the cycle from the path
        const cycleStart = path.indexOf(neighbor);
        cycle = path.slice(cycleStart);
        return true;
      }
      
      // If the neighbor hasn't been visited, visit it
      if (!visited[neighbor] && detectCycle(neighbor, path)) {
        return true;
      }
    }
    
    // Remove the node from recursion stack and path
    path.pop();
    recStack[node] = false;
    return false;
  }
  
  // Try to find a cycle starting from each node
  for (const node in waitForGraph) {
    if (!visited[node]) {
      if (detectCycle(parseInt(node))) {
        break;
      }
    }
  }
  
  return cycle;
}

/**
 * Generate graph state from logs' cumulative effect
 *
 * @param {Array} logs - Log array created by transformLogs
 * @param {Object} graphThreadMapping - { raw_thread_id: incrementalNum } e.g. {6164146352: 1, 6166292656: 2}
 * @param {Object} resourceMapping - { raw_lock: letter } e.g. {1:"A", 2:"B"}
 *
 * In graph state:
 * - Thread node ids are "T" + thread_id (e.g. "T123")
 * - Resource nodes are "R" + resource_id (e.g. "R1")
 */
function generateGraphStateFromLogs(logs, graphThreadMapping, resourceMapping) {
  // Initialize empty collections for active nodes and links
  const activeThreads = new Set();
  const activeResources = new Set();
  const graphStates = [];
  
  // Create mappings to track nodes and links
  const threadNodes = Object.keys(graphThreadMapping).map(threadId => ({
    id: `T${threadId}`,
    name: `Thread ${threadId}`,
      type: "thread",
  }));
  
  const resourceNodes = Object.keys(resourceMapping).map((lockNum) => {
    return {
      id: `R${lockNum}`,
      name: `Resource ${resourceMapping[lockNum]}`,
      type: "resource",
    };
  });
  
  // Map of all possible nodes by id
  const nodesMap = {};
  threadNodes.forEach(node => {
    nodesMap[node.id] = node;
  });
  resourceNodes.forEach(node => {
    nodesMap[node.id] = node;
  });
  
  // First state: empty graph with no nodes or links
  graphStates.push({ 
    step: 1, 
    nodes: [], 
    links: [] 
  });

  // Cumulative link state: key format is "T{thread_id}-R{resource_id}"
  const cumulativeLinks = {};
  
  // Process each log event to build the graph state incrementally
  logs.forEach((log, idx) => {
    if (log.type === "init") return; // Skip the init log
    
    const prevState = graphStates[graphStates.length - 1];
    const currentNodes = [...prevState.nodes];
    let currentLinks = [...prevState.links];
    
    // Handle spawn events
    if (log.type === "spawn") {
      if (log.thread_id !== 0) {
        // Thread spawn
        const threadId = log.thread_id;
        const nodeId = `T${threadId}`;
        
        // Add thread to active set and to nodes if not already there
        if (!activeThreads.has(nodeId)) {
          activeThreads.add(nodeId);
          // Create node if it doesn't exist in the map
          if (!nodesMap[nodeId]) {
            nodesMap[nodeId] = {
              id: nodeId,
              name: `Thread ${threadId}`,
              type: "thread",
            };
          }
          currentNodes.push(nodesMap[nodeId]);
        }
      } else if (log.resource_id) {
        // Resource creation
        const resourceId = `R${log.resource_id.replace(/^[A-Z]/, '')}`;
        
        // Add resource to active set and to nodes if not already there
        if (!activeResources.has(resourceId)) {
          activeResources.add(resourceId);
          // Create node if it doesn't exist in the map
          if (!nodesMap[resourceId]) {
            nodesMap[resourceId] = {
              id: resourceId,
              name: `Resource ${log.resource_id}`,
              type: "resource",
            };
          }
          currentNodes.push(nodesMap[resourceId]);
        }
      }
    }
    // Handle exit events
    else if (log.type === "exit") {
      if (log.thread_id !== 0) {
        // Thread exit
        const threadId = log.thread_id;
        const nodeId = `T${threadId}`;
        
        // Remove thread from active set and from nodes
        activeThreads.delete(nodeId);
        const nodeIndex = currentNodes.findIndex(n => n.id === nodeId);
        if (nodeIndex !== -1) {
          currentNodes.splice(nodeIndex, 1);
        }
        
        // Remove any links connected to this thread
        Object.keys(cumulativeLinks).forEach(key => {
          if (key.startsWith(`${nodeId}-`)) {
            delete cumulativeLinks[key];
          }
        });
        
        // Update links array to reflect removed links
        currentLinks = Object.keys(cumulativeLinks).map(key => {
          const [source, target] = key.split("-");
          return { source, target, type: cumulativeLinks[key] };
        });
      } else if (log.resource_id) {
        // Resource removal
        const resourceId = `R${log.resource_id.replace(/^[A-Z]/, '')}`;
        
        // Remove resource from active set and from nodes
        activeResources.delete(resourceId);
        const nodeIndex = currentNodes.findIndex(n => n.id === resourceId);
        if (nodeIndex !== -1) {
          currentNodes.splice(nodeIndex, 1);
        }
        
        // Remove any links connected to this resource
        Object.keys(cumulativeLinks).forEach(key => {
          if (key.endsWith(`-${resourceId}`)) {
            delete cumulativeLinks[key];
          }
        });
        
        // Update links array to reflect removed links
        currentLinks = Object.keys(cumulativeLinks).map(key => {
          const [source, target] = key.split("-");
          return { source, target, type: cumulativeLinks[key] };
        });
      }
    }
    // Handle resource access events (attempt, acquired, released)
    else if (["attempt", "acquired", "released"].includes(log.type) && 
             log.thread_id !== 0 && log.resource_id) {
      const threadId = log.thread_id;
      const sourceId = `T${threadId}`;
      const resourceIdStr = log.resource_id.toString().replace(/^[A-Z]/, '');
      const targetId = `R${resourceIdStr}`;
      const linkKey = `${sourceId}-${targetId}`;
      
      // Make sure both nodes exist in the active sets
      if (!activeThreads.has(sourceId)) {
        activeThreads.add(sourceId);
        // Create node if it doesn't exist in the map
        if (!nodesMap[sourceId]) {
          nodesMap[sourceId] = {
            id: sourceId,
            name: `Thread ${threadId}`,
            type: "thread",
          };
        }
        currentNodes.push(nodesMap[sourceId]);
      }
      
      if (!activeResources.has(targetId)) {
        activeResources.add(targetId);
        // Create node if it doesn't exist in the map
        if (!nodesMap[targetId]) {
          nodesMap[targetId] = {
            id: targetId,
            name: `Resource ${log.resource_id}`,
            type: "resource",
          };
        }
        currentNodes.push(nodesMap[targetId]);
      }
      
      // Update link state
      if (log.type === "released") {
      // Remove the link when resource is released
        delete cumulativeLinks[linkKey];
    } else {
      // Add or update link for attempt or acquired
        cumulativeLinks[linkKey] = log.type;
    }

    // Convert current link state to array for D3
      currentLinks = Object.keys(cumulativeLinks).map(key => {
        const [s, t] = key.split("-");
        return { source: s, target: t, type: cumulativeLinks[key] };
      });
    }
    // Handle deadlock event
    else if (log.type === "deadlock") {
      // Mark nodes in the deadlock cycle if exists
      if (log.cycle && log.cycle.length >= 2) {
        // Mark all threads in the deadlock cycle
        log.cycle.forEach(threadId => {
          const nodeId = `T${threadId}`;
          const node = currentNodes.find(n => n.id === nodeId);
          if (node) {
            node.inDeadlock = true;
          }
        });
      }
    }
    
    // Add the current state to graph states
    graphStates.push({
      step: graphStates.length + 1,
      nodes: currentNodes,
      links: currentLinks,
    });
  });

  // If the last event was a deadlock, add deadlock links between threads in the cycle
  const deadlockLog = logs.find(log => log.type === "deadlock");
  if (deadlockLog && deadlockLog.cycle && deadlockLog.cycle.length >= 2) {
    console.log("DEADLOCK DETECTED - Creating deadlock links", deadlockLog);
    const lastState = graphStates[graphStates.length - 1];
    const deadlockLinks = [...lastState.links];
    const deadlockThreads = deadlockLog.cycle;
    
    console.log("Deadlock threads in cycle:", deadlockThreads);
    
    // Get thread waiting for resource information
    const waitingForInfo = deadlockLog.deadlock_details.thread_waiting_for_locks;
    console.log("Thread waiting for locks info:", waitingForInfo);
    
    // Create a mapping from thread to resource it's waiting for
    const threadToResource = {};
    waitingForInfo.forEach(info => {
      threadToResource[info.thread_id] = info.resource_id;
    });
    console.log("Thread to resource mapping:", threadToResource);
    
    // Create a mapping from resource to thread holding it
    const resourceToThread = {};
    
    // Find resource owners by checking acquired links in the current state
    lastState.links.forEach(link => {
      if (link.type === "acquired" && link.source.startsWith("T") && link.target.startsWith("R")) {
        const threadId = link.source.substring(1); // Remove 'T' prefix
        const resourceId = link.target.substring(1); // Remove 'R' prefix
        resourceToThread[resourceId] = threadId;
      }
    });
    console.log("Resource to thread mapping:", resourceToThread);
    
    // Create deadlock links directly between threads in the cycle
    for (let i = 0; i < deadlockThreads.length; i++) {
      const currentThread = deadlockThreads[i];
      const nextThread = deadlockThreads[(i + 1) % deadlockThreads.length];
      
      console.log(`Creating deadlock link: T${currentThread} -> T${nextThread}`);
      
      // Find the actual node objects instead of just using strings
      const sourceNode = lastState.nodes.find(node => node.id === `T${currentThread}`);
      const targetNode = lastState.nodes.find(node => node.id === `T${nextThread}`);
      
      if (sourceNode && targetNode) {
        // Add direct thread-to-thread deadlock link with proper object references
      deadlockLinks.push({
          source: sourceNode,
          target: targetNode,
        type: "deadlock",
          isDeadlockEdge: true
        });
        console.log("Added deadlock link with object references");
  } else {
        console.error("Could not find nodes for threads:", currentThread, nextThread);
        console.log("Available nodes:", lastState.nodes.map(n => n.id));
      }
    }
    
    console.log("Final deadlock links count:", deadlockLinks.length);
    
    // Create the final graph state with the deadlock cycles
  graphStates.push({
    step: graphStates.length + 1,
      nodes: lastState.nodes.map(node => ({...node})), // Create a deep copy to avoid reference issues
    links: deadlockLinks,
    });

    console.log("Added new graph state with deadlock links");
  }

  return graphStates;
}

/**
 * Transform raw object into logs and graph_state arrays
 *
 * rawData: [ rawLogs, rawGraph ]
 * Here rawGraph is not used; the graph state is derived from logs
 */
function transformRawObject(rawData) {
  const rawLogs = rawData[0];

  // For graph: Keep track of thread IDs without re-mapping
  const graphThreadMapping = {};
  rawLogs.forEach((log) => {
    const rawThread = log[0];
    if (rawThread !== 0 && !(rawThread in graphThreadMapping)) {
      graphThreadMapping[rawThread] = rawThread;
    }
  });

  // Resource mapping: Convert raw lock number to string representation
  const resourceMapping = {};
  rawLogs.forEach((log) => {
    const lockNum = log[1];
    if (lockNum !== 0 && !(lockNum in resourceMapping)) {
      resourceMapping[lockNum] = mapLockId(lockNum);
    }
  });

  // Transform logs and generate graph states
  const logs = transformLogs(rawLogs, resourceMapping);
  const graph_state = generateGraphStateFromLogs(
    logs,
    graphThreadMapping,
    resourceMapping
  );

  return { logs, graph_state };
}

/**
 * Decode logs from URL-safe Base64, Gzip and MessagePack encoded string
 */
function decodeLogs(encodedStr) {
  try {
    if (typeof encodedStr !== 'string') {
      throw new Error("encodedStr must be a string");
    }
    
    var base64 = encodedStr.replace(/-/g, "+").replace(/_/g, "/");
    var binaryStr = atob(base64);
    var len = binaryStr.length;
    var bytes = new Uint8Array(len);
    
  for (var i = 0; i < len; i++) {
      bytes[i] = binaryStr.charCodeAt(i);
  }
    
    var decompressed = pako.ungzip(bytes);
    var logsData = msgpack.decode(decompressed);
    
    return logsData;
  } catch (error) {
    console.error("Error decoding logs:", error);
    throw new Error("Failed to decode the logs data: " + error.message);
  }
}

/**
 * Process encoded log from URL and return transformed data
 */
function processEncodedLog(encodedStr) {
  try {
    // Handle case where encodedStr is already an object (not a string)
    if (typeof encodedStr !== 'string') {
      return transformRawObject(encodedStr);
    }
    
    const decoded = decodeLogs(encodedStr);
    return transformRawObject(decoded);
  } catch (error) {
    console.error("Error in processEncodedLog:", error);
    throw error;
  }
}

/**
 * Process logs in the new format (one JSON object per line)
 *
 * @param {string} logText - Raw text containing one JSON object per line
 * @returns {Object} - Structured logs and graph state data
 */
function processNewFormatLogs(logText) {
  try {
    let jsonData;
    
    // Check if the input is already a parsed object or a string that needs parsing
    if (typeof logText === 'string') {
      // Handle the line-by-line JSON format
      if (logText.trim().startsWith("{") && logText.includes('{"event":')) {
        // Split by newlines and parse each line as a separate JSON object
        const lines = logText.trim().split('\n');
        const events = [];
        const graphState = [];
        
        // Keep track of deadlock detection
        let deadlockDetected = false;
        let deadlockThreads = [];
        
        // Process each line as a separate JSON object
        for (let i = 0; i < lines.length; i++) {
          const line = lines[i];
          if (!line.trim()) continue; // Skip empty lines
          
          try {
            const lineData = JSON.parse(line.trim());
            if (lineData.event) {
              const { thread_id, lock_id, event, timestamp, parent_id } = lineData.event;
              
              // Check if this event would create a deadlock
              if (event === 'Attempt' && lineData.graph) {
                const links = lineData.graph.links || [];
                // If we have a deadlock condition (thread attempting to acquire a resource held by another waiting thread)
                const attemptLinks = links.filter(link => link.type === 'Attempt');
                const acquiredLinks = links.filter(link => link.type === 'Acquired');
                
                // Simple deadlock detection: If we have at least 2 threads where each is waiting for a resource 
                // held by the other, we have a deadlock
                if (attemptLinks.length >= 2) {
                  // Check for cyclic dependencies
                  const dependencies = {};
                  
                  // Build dependency graph
                  for (const link of links) {
                    if (link.type === 'Acquired') {
                      // A thread has acquired a resource
                      const threadId = link.source;
                      const resourceId = link.target;
                      
                      // Mark this resource as being held by this thread
                      if (!dependencies[resourceId]) {
                        dependencies[resourceId] = { heldBy: threadId };
                      } else {
                        dependencies[resourceId].heldBy = threadId;
                      }
                    } else if (link.type === 'Attempt') {
                      // A thread is waiting for a resource
                      const threadId = link.source;
                      const resourceId = link.target;
                      
                      // Mark this thread as waiting for this resource
                      if (!dependencies[resourceId]) {
                        dependencies[resourceId] = { waitingThreads: [threadId] };
                      } else if (!dependencies[resourceId].waitingThreads) {
                        dependencies[resourceId].waitingThreads = [threadId];
                      } else {
                        dependencies[resourceId].waitingThreads.push(threadId);
                      }
                    }
                  }
                  
                  // Check for cycles in the dependency graph
                  const visited = new Set();
                  const recStack = new Set();
                  const threadsInCycle = new Set();
                  
                  // Function to detect cycle
                  const detectCycle = (threadId, path = []) => {
                    if (recStack.has(threadId)) {
                      // We found a cycle
                      const cycleStart = path.indexOf(threadId);
                      deadlockThreads = path.slice(cycleStart);
                      deadlockThreads.forEach(t => threadsInCycle.add(t));
                      return true;
                    }
                    
                    if (visited.has(threadId)) {
                      return false;
                    }
                    
                    visited.add(threadId);
                    recStack.add(threadId);
                    path.push(threadId);
                    
                    // Find resources this thread is waiting for
                    for (const [resourceId, info] of Object.entries(dependencies)) {
                      if (info.waitingThreads && info.waitingThreads.includes(threadId) && info.heldBy) {
                        // This thread is waiting for a resource held by another thread
                        if (detectCycle(info.heldBy, [...path])) {
                          return true;
                        }
                      }
                    }
                    
                    recStack.delete(threadId);
                    return false;
                  };
                  
                  // Start cycle detection from each thread that's waiting
                  for (const [resourceId, info] of Object.entries(dependencies)) {
                    if (info.waitingThreads && info.waitingThreads.length > 0 && info.heldBy) {
                      for (const waitingThread of info.waitingThreads) {
                        if (detectCycle(waitingThread)) {
                          deadlockDetected = true;
                          break;
                        }
                      }
                      if (deadlockDetected) break;
                    }
                  }
                  
                  // If we've detected a deadlock, we want this to be the last event we process
                  if (deadlockDetected && threadsInCycle.size >= 2) {
                    console.log("Deadlock detected, stopping log processing");
                    // Add the current event that caused the deadlock
                    const formattedEvent = [
                      thread_id, 
                      lock_id, 
                      getEventCode(event), 
                      timestamp,
                      parent_id || 0
                    ];
                    events.push(formattedEvent);
                    
                    // Add the current graph state
                    if (lineData.graph) {
                      graphState.push(lineData.graph);
                    }
                    
                    // Break out of the loop to stop processing further logs
                    break;
                  }
                }
              }
              
              // Convert event to event code
              const eventCode = getEventCode(event);
              
              // Create event in the expected format [thread_id, lock_id, event_code, timestamp, parent_id]
              const formattedEvent = [
                thread_id, 
                lock_id, 
                eventCode, 
                timestamp,
                parent_id || 0
              ];
              
              events.push(formattedEvent);
              
              // Also store the graph state if available
              if (lineData.graph) {
                graphState.push(lineData.graph);
              }
            }
          } catch (lineError) {
            console.error("Error parsing JSON line:", lineError, line);
            // Continue with next line instead of failing completely
          }
        }
        
        // Create the data structure expected by the rest of the code
        jsonData = { events, graphs: graphState };
      } else {
        // Try to parse as a single JSON object
        jsonData = JSON.parse(logText);
      }
    } else {
      jsonData = logText;
    }
    
    // Process raw logs in the format [thread_id, lock_id, event_code, timestamp, parent_id]
    const rawLogs = Array.isArray(jsonData.events) ? jsonData.events : [];

    // Get all unique thread IDs and lock IDs from the events
    const allThreads = new Set();
    const allLocks = new Set();

    rawLogs.forEach(log => {
      const threadId = log[0];
      const lockId = log[1];
      
      if (threadId !== 0) {
        allThreads.add(threadId);
      }
      if (lockId !== 0) {
        allLocks.add(lockId);
      }
    });

    // Create resource mapping (use lock_id directly)
    const resourceMapping = {};
    Array.from(allLocks).forEach((lockId) => {
      resourceMapping[lockId] = mapLockId(lockId);
    });

    // Create thread mapping (use thread_id directly)
    const graphThreadMapping = {};
    Array.from(allThreads).forEach((threadId) => {
      graphThreadMapping[threadId] = threadId;
    });

    // Transform raw logs into structured log entries
    const logs = transformLogs(rawLogs, resourceMapping);

    // Generate graph states based on logs
    const graphStates = generateGraphStateFromLogs(
      logs,
      graphThreadMapping,
      resourceMapping
    );

    return {
      logs,
      graph_state: graphStates,
    };
  } catch (error) {
    console.error("Error processing logs:", error);
    throw error;
  }
}

// Helper function to convert event string to event code
function getEventCode(event) {
  switch (event) {
    case 'Attempt': return 0;
    case 'Acquired': return 1;
    case 'Released': return 2;
    case 'Spawn': return 3;
    case 'Exit': return 4;
    default: return -1; // Unknown event
  }
}
