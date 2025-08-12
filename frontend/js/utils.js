/**
 * Deloxide - Deadlock Detection Visualization Utilities
 *
 * This file contains utility functions for processing and transforming
 * raw deadlock log data into a format usable by the visualization.
 */

// Helper functions for event classification (used globally)
function getLockType(eventType) {
  if (eventType.includes("mutex")) return "mutex";
  if (eventType.includes("rwlock")) return "rwlock";
  if (eventType.includes("condvar")) return "condvar";
  return "mutex"; // default fallback
}

function isAcquisitionEvent(eventType) {
  return eventType.includes("acquired") || eventType === "acquired";
}

function isAttemptEvent(eventType) {
  return eventType.includes("attempt") || eventType === "attempt";
}

function isReleaseEvent(eventType) {
  return eventType.includes("released") || eventType === "released";
}

function isWaitBeginEvent(eventType) {
  return eventType.includes("wait_begin");
}

function isWaitEndEvent(eventType) {
  return eventType.includes("wait_end");
}

function isSpawnEvent(eventType) {
  return eventType.includes("spawn") || eventType === "spawn";
}

function isExitEvent(eventType) {
  return eventType.includes("exit") || eventType === "exit";
}

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
    // Condvar notify (transient visual cue only)
    else if ((log.type && log.type.toLowerCase().includes("notify")) && log.thread_id !== 0 && log.resource_id) {
      const threadId = log.thread_id;
      const sourceId = `T${threadId}`;
      const resourceLetter = log.resource_id;
      const resourceNum = Object.keys(resourceMapping).find(key => resourceMapping[key] === resourceLetter);
      const targetId = `R${resourceNum}`;

      if (!activeThreads.has(sourceId)) {
        activeThreads.add(sourceId);
        if (!nodesMap[sourceId]) {
          nodesMap[sourceId] = { id: sourceId, name: `Thread ${threadId}`, type: "thread" };
        }
        currentNodes.push(nodesMap[sourceId]);
      }
      if (!activeResources.has(targetId)) {
        activeResources.add(targetId);
        if (!nodesMap[targetId]) {
          nodesMap[targetId] = { id: targetId, name: `Resource ${log.resource_id}`, type: "resource" };
        }
        currentNodes.push(nodesMap[targetId]);
      }

      transientLinks.push({ source: sourceId, target: targetId, type: "notify" });
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
  
  // Event type mapping with unique codes for each event type
  const eventTypes = {
    // Thread lifecycle
    0: "thread_spawn",
    1: "thread_exit",
    
    // Mutex lifecycle 
    2: "mutex_spawn",
    3: "mutex_exit",
    
    // RwLock lifecycle
    4: "rwlock_spawn",
    5: "rwlock_exit",
    
    // Condvar lifecycle
    6: "condvar_spawn",
    7: "condvar_exit",
    
    // Mutex interactions
    10: "mutex_attempt",
    11: "mutex_acquired",
    12: "mutex_released",
    
    // RwLock interactions
    20: "rwlock_read_attempt",
    21: "rwlock_read_acquired",
    22: "rwlock_read_released",
    23: "rwlock_write_attempt",
    24: "rwlock_write_acquired",
    25: "rwlock_write_released",
    
    // Condvar interactions
    30: "condvar_wait_begin",
    31: "condvar_wait_end",
    32: "condvar_notify_one",
    33: "condvar_notify_all",
    
    // Legacy generic events for backward compatibility
    40: "attempt",
    41: "acquired",
    42: "released"
  };
  
  // Keep track of resource ownership and waiting threads for deadlock detection
  const resourceOwners = {}; // Maps resource_id to thread_id that owns it
  const threadWaiting = {}; // Maps thread_id to resource_id it's waiting for
  // Per-condvar wait queues to infer which waiter is woken on notify_one/all
  const condvarQueues = {}; // Maps raw condvar lockNum -> Array<threadId>
  
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

    // Maintain condvar wait queues to infer woken thread(s)
    if (type === "condvar_wait_begin" && lockNum !== 0 && rawThread !== 0) {
      if (!condvarQueues[lockNum]) condvarQueues[lockNum] = [];
      condvarQueues[lockNum].push(rawThread);
    } else if (type === "condvar_wait_end" && lockNum !== 0 && rawThread !== 0) {
      const q = condvarQueues[lockNum];
      if (q) {
        const idxq = q.indexOf(rawThread);
        if (idxq !== -1) q.splice(idxq, 1);
      }
    }
    
    // Update resource ownership tracking for deadlock detection
    if (isAcquisitionEvent(type) && rawThread !== 0 && lockNum !== 0) {
      resourceOwners[lockNum] = rawThread;
      // Thread is no longer waiting
      delete threadWaiting[rawThread];
    }
    else if (isAttemptEvent(type) && rawThread !== 0 && lockNum !== 0) {
      // Thread is now waiting for this resource
      threadWaiting[rawThread] = lockNum;
      
      // Check for deadlock after every attempt event
      const deadlockCycle = detectDeadlockCycle(resourceOwners, threadWaiting);
      if (deadlockCycle && deadlockCycle.length >= 2) {
        deadlockDetected = true;
      }
    }
    else if (isWaitBeginEvent(type) && rawThread !== 0 && lockNum !== 0) {
      // Treat wait_begin as entering waiting state on the associated mutex
      threadWaiting[rawThread] = lockNum;
      const deadlockCycle = detectDeadlockCycle(resourceOwners, threadWaiting);
      if (deadlockCycle && deadlockCycle.length >= 2) {
        deadlockDetected = true;
      }
    }
    else if (isReleaseEvent(type) && rawThread !== 0 && lockNum !== 0) {
      // Resource is no longer owned by this thread
      if (resourceOwners[lockNum] === rawThread) {
        delete resourceOwners[lockNum];
      }
    }
    else if (isWaitEndEvent(type) && rawThread !== 0) {
      // Leaving wait: clear waiting state (mutex reacquired will set owner)
      delete threadWaiting[rawThread];
    }
    
    // For spawn events: Either a thread is spawned or a resource is created
    if (isSpawnEvent(type)) {
      let description = "";
      const lockType = getLockType(type);
      
      if (type === "thread_spawn" && rawThread !== 0) {
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
        // Lock/Resource creation
        let parentName;
        if (parentId === 0) {
          parentName = "main thread";
        } else if (parentId === mainThreadId) {
          parentName = `<span class="main-thread">Main Thread</span>`;
        } else {
          parentName = `<span class="thread-id">Thread ${parentId}</span>`;
        }
        
        let lockTypeDesc = "";
        switch (lockType) {
          case "mutex": lockTypeDesc = "Mutex"; break;
          case "rwlock": lockTypeDesc = "RwLock"; break;
          case "condvar": lockTypeDesc = "Condvar"; break;
          default: lockTypeDesc = "Resource"; break;
        }
        
        description = `<span class="resource-id ${lockType}">${lockTypeDesc} ${resourceMapping[lockNum]}</span> created by ${parentName}.`;
      }
      
      logs.push({
      step: idx + 2, // init step is 1, so we start from 2
      timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
        type,
        lock_type: lockType,
        thread_id: rawThread,
        resource_id: lockNum !== 0 ? resourceMapping[lockNum] : null,
        parent_id: parentId,
        description,
        is_main_thread: rawThread === mainThreadId, // Mark if this is the main thread
      });
    } 
    // For exit events: Either a thread exits or a resource is dropped
    else if (isExitEvent(type)) {
      let description = "";
      const lockType = getLockType(type);
      
      if (type === "thread_exit" && rawThread !== 0) {
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
        let lockTypeDesc = "";
        switch (lockType) {
          case "mutex": lockTypeDesc = "Mutex"; break;
          case "rwlock": lockTypeDesc = "RwLock"; break;
          case "condvar": lockTypeDesc = "Condvar"; break;
          default: lockTypeDesc = "Resource"; break;
        }
        
        description = `<span class="resource-id ${lockType}">${lockTypeDesc} ${resourceMapping[lockNum]}</span> dropped.`;
        
        // Clean up tracking for this resource
        delete resourceOwners[lockNum];
      }
      
      logs.push({
        step: idx + 2, // init step is 1, so we start from 2
        timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
        type,
        lock_type: lockType,
        thread_id: rawThread,
        resource_id: lockNum !== 0 ? resourceMapping[lockNum] : null,
        description,
        is_main_thread: rawThread === mainThreadId, // Mark if this is the main thread
      });
    }
    // For interaction events (attempt, acquired, released, notify, wait)
    else {
      let threadDescription;
      if (rawThread === mainThreadId) {
        threadDescription = `<span class="main-thread">Main Thread</span>`;
      } else {
        threadDescription = `<span class="thread-id">Thread ${rawThread}</span>`;
      }
      
      const lockType = getLockType(type);
      
      // Generate action description based on event type
      let actionText = "";
      let lockTypeDesc = "";
      let extraSuffix = ""; // for notify target info
      
      switch (lockType) {
        case "mutex": lockTypeDesc = "Mutex"; break;
        case "rwlock": lockTypeDesc = "RwLock"; break;
        case "condvar": lockTypeDesc = "Condvar"; break;
        default: lockTypeDesc = "Resource"; break;
      }
      
      switch (type) {
        // Mutex events
        case "mutex_attempt":
        case "attempt":
          actionText = `attempted to acquire`;
          break;
        case "mutex_acquired":
        case "acquired":
          actionText = `acquired`;
          break;
        case "mutex_released":
        case "released":
          actionText = `released`;
          break;
          
        // RwLock events
        case "rwlock_read_attempt":
          actionText = `attempted to read-lock`;
          break;
        case "rwlock_read_acquired":
          actionText = `acquired read-lock on`;
          break;
        case "rwlock_read_released":
          actionText = `released read-lock on`;
          break;
        case "rwlock_write_attempt":
          actionText = `attempted to write-lock`;
          break;
        case "rwlock_write_acquired":
          actionText = `acquired write-lock on`;
          break;
        case "rwlock_write_released":
          actionText = `released write-lock on`;
          break;
          
        // Condvar events
        case "condvar_wait_begin":
          actionText = `began waiting on`;
          break;
        case "condvar_wait_end":
          actionText = `finished waiting on`;
          break;
        case "condvar_notify_one":
          actionText = `notified one waiter on`;
          {
            const q = condvarQueues[lockNum] || [];
            const woken = q.length > 0 ? q[0] : null; // next to be woken (we'll pop when event is committed)
            if (woken) {
              const wokenDisplay = woken === mainThreadId ? `<span class="main-thread">Main Thread</span>` : `<span class="thread-id">Thread ${woken}</span>`;
              extraSuffix = ` (woke ${wokenDisplay})`;
            } else {
              extraSuffix = ` (no waiters)`;
            }
          }
          break;
        case "condvar_notify_all":
          actionText = `notified all waiters on`;
          {
            const q = condvarQueues[lockNum] || [];
            if (q.length > 0) {
              const list = q.map(t => t === mainThreadId ? `Main Thread` : `Thread ${t}`).join(", ");
              extraSuffix = ` (woke ${list})`;
            } else {
              extraSuffix = ` (no waiters)`;
            }
          }
          break;
          
        default:
          actionText = type;
          break;
      }
      
      // Build base log entry
      const baseEntry = {
        step: idx + 2, // init step is 1, so we start from 2
        timestamp: Math.floor(timestamp * 1000), // Convert to milliseconds
        type,
        lock_type: lockType,
        thread_id: rawThread,
        resource_id: resourceMapping[lockNum],
        description: `${threadDescription} ${actionText} <span class="resource-id ${lockType}">${lockTypeDesc} ${resourceMapping[lockNum]}</span>${extraSuffix}`,
        is_main_thread: rawThread === mainThreadId, // Mark if this is the main thread
      };

      // Attach explicit woken info and update queues on notify events
      if (type === "condvar_notify_one") {
        const q = condvarQueues[lockNum] || [];
        const woken = q.length > 0 ? q.shift() : null;
        if (woken) baseEntry.woken_thread_id = woken;
      } else if (type === "condvar_notify_all") {
        const q = condvarQueues[lockNum] || [];
        if (q.length > 0) {
          baseEntry.woken_threads = q.slice();
          q.length = 0; // clear
        } else {
          baseEntry.woken_threads = [];
        }
      }

      logs.push(baseEntry);
    }
  }

  // Remove frontend deadlock detection; rely on terminal record in logs

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
      lock_type: "mutex", // default, will be updated when resource is created
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
    // Transient links that exist only for this step (e.g., notify)
    const transientLinks = [];
    
    // Handle spawn events
    if (isSpawnEvent(log.type)) {
      if (log.type === "thread_spawn" && log.thread_id !== 0) {
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
        // Resource creation (Mutex, RwLock, or Condvar)
        const resourceId = `R${log.resource_id.replace(/^[A-Z]/, '')}`;
        const lockType = log.lock_type || "mutex";
        
        // Add resource to active set and to nodes if not already there
        if (!activeResources.has(resourceId)) {
          activeResources.add(resourceId);
          
          let lockTypeDesc = "";
          switch (lockType) {
            case "mutex": lockTypeDesc = "Mutex"; break;
            case "rwlock": lockTypeDesc = "RwLock"; break;
            case "condvar": lockTypeDesc = "Condvar"; break;
            default: lockTypeDesc = "Resource"; break;
          }
          
          // Create node if it doesn't exist in the map
          if (!nodesMap[resourceId]) {
            nodesMap[resourceId] = {
              id: resourceId,
              name: `${lockTypeDesc} ${log.resource_id}`,
              type: "resource",
              lock_type: lockType,
            };
          } else {
            // Update existing node with lock type info
            nodesMap[resourceId].lock_type = lockType;
            nodesMap[resourceId].name = `${lockTypeDesc} ${log.resource_id}`;
          }
          currentNodes.push(nodesMap[resourceId]);
        }
      }
    }
    // Check if this is an exit event (any type)
    // Handle exit events
    else if (isExitEvent(log.type)) {
      if (log.type === "thread_exit" && log.thread_id !== 0) {
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
    // Handle resource access events (attempt, acquired, released, wait)
    else if ((isAttemptEvent(log.type) || isWaitBeginEvent(log.type) || isAcquisitionEvent(log.type) || isReleaseEvent(log.type) || isWaitEndEvent(log.type)) && 
             log.thread_id !== 0 && log.resource_id) {
      const threadId = log.thread_id;
      const sourceId = `T${threadId}`;
      // Find the numeric key for this resource letter from resourceMapping
      const resourceLetter = log.resource_id;
      const resourceNum = Object.keys(resourceMapping).find(key => resourceMapping[key] === resourceLetter);
      const targetId = `R${resourceNum}`;
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
      if (isReleaseEvent(log.type) || isWaitEndEvent(log.type)) {
      // Remove the link when resource is released
        delete cumulativeLinks[linkKey];
    } else {
        // Add or update link for attempt, wait_begin or acquired
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
      links: [...currentLinks, ...transientLinks],
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
  // Support new format: { events, deadlock }
  try { console.log('[transformRawObject] rawData keys:', Array.isArray(rawData) ? 'array' : Object.keys(rawData || {})); } catch (e) {}
  const rawLogs = Array.isArray(rawData)
    ? rawData[0]
    : (Array.isArray(rawData.events) ? rawData.events : []);

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
  // If backend provided terminal deadlock info, append it and skip internal detection
  try {
    if (Array.isArray(rawData)) {
      const second = (rawData.length > 1) ? rawData[1] : null;
      let dl = null;
      if (second && typeof second === 'object' && !Array.isArray(second)) {
        dl = second.deadlock ? second.deadlock : (Array.isArray(second.thread_cycle) ? second : null);
      } else if (Array.isArray(second) && second.length >= 3) {
        // rmp_serde may encode struct as an array: [thread_cycle, thread_waiting_for_locks, timestamp]
        const [thread_cycle, thread_waiting_for_locks, timestamp] = second;
        if (Array.isArray(thread_cycle)) {
          dl = { thread_cycle, thread_waiting_for_locks: thread_waiting_for_locks || [], timestamp };
          console.log('[transformRawObject] Interpreted tuple-form deadlock:', dl);
        }
      }
      if (dl && Array.isArray(dl.thread_cycle)) {
        console.log('[transformRawObject] Found terminal deadlock (array form):', dl);

        // Build rich description
        const waits = (dl.thread_waiting_for_locks || []).map(w => Array.isArray(w) ? { thread_id: w[0], resource_id: w[1] } : w);
        const resourceTypeById = {};
        try {
          logs.forEach(e => {
            if (e && typeof e.type === 'string' && e.type.endsWith('_spawn') && e.resource_id) {
              const num = parseInt(e.resource_id);
              if (!isNaN(num)) resourceTypeById[num] = e.lock_type || 'mutex';
            }
          });
        } catch (e) { /* ignore */ }
        const descLines = [];
        for (let i = 0; i < dl.thread_cycle.length; i++) {
          const threadId = dl.thread_cycle[i];
          const nextThread = dl.thread_cycle[(i + 1) % dl.thread_cycle.length];
          const wait = waits.find(w => w.thread_id === threadId);
          if (!wait) continue;
          const resId = parseInt(wait.resource_id);
          const rType = resourceTypeById[resId] || 'mutex';
          const rLabel = rType === 'rwlock' ? 'RwLock' : (rType === 'condvar' ? 'Condvar' : 'Mutex');
          descLines.push(`<span class="thread-id">Thread ${threadId}</span> is waiting for <span class="resource-id ${rType}">${rLabel} ${resId}</span> held by <span class="thread-id">Thread ${nextThread}</span>`);
        }
        const description = `<strong>DEADLOCK DETECTED:</strong><br>` + (descLines.length ? descLines.join('<br>') : '');

        logs.push({
          step: logs.length + 1,
          timestamp: Date.now(),
          type: 'deadlock',
          cycle: dl.thread_cycle,
          description,
          deadlock_details: {
            thread_cycle: dl.thread_cycle,
            thread_waiting_for_locks: waits.map(w => ({ thread_id: w.thread_id, lock_id: w.resource_id, resource_id: w.resource_id })),
            timestamp: dl.timestamp,
          },
        });
        console.log('[transformRawObject] Appended deadlock entry (array) at step', logs[logs.length-1].step);
      } else {
        console.log('[transformRawObject] No terminal deadlock found in array payload second element:', second);
      }
    } else {
      const dl = (rawData && rawData.deadlock) ? rawData.deadlock : null;
      if (dl) {
        console.log('[transformRawObject] Found terminal deadlock (object form):', dl);
        // Build rich description
        const waits = (dl.thread_waiting_for_locks || []).map(w => Array.isArray(w) ? { thread_id: w[0], resource_id: w[1] } : w);
        const resourceTypeById = {};
        try {
          logs.forEach(e => {
            if (e && typeof e.type === 'string' && e.type.endsWith('_spawn') && e.resource_id) {
              const num = parseInt(e.resource_id);
              if (!isNaN(num)) resourceTypeById[num] = e.lock_type || 'mutex';
            }
          });
        } catch (e) { /* ignore */ }
        const descLines = [];
        for (let i = 0; i < dl.thread_cycle.length; i++) {
          const threadId = dl.thread_cycle[i];
          const nextThread = dl.thread_cycle[(i + 1) % dl.thread_cycle.length];
          const wait = waits.find(w => w.thread_id === threadId);
          if (!wait) continue;
          const resId = parseInt(wait.resource_id);
          const rType = resourceTypeById[resId] || 'mutex';
          const rLabel = rType === 'rwlock' ? 'RwLock' : (rType === 'condvar' ? 'Condvar' : 'Mutex');
          descLines.push(`<span class="thread-id">Thread ${threadId}</span> is waiting for <span class="resource-id ${rType}">${rLabel} ${resId}</span> held by <span class="thread-id">Thread ${nextThread}</span>`);
        }
        const description = `<strong>DEADLOCK DETECTED:</strong><br>` + (descLines.length ? descLines.join('<br>') : '');

        logs.push({
          step: logs.length + 1,
          timestamp: Date.now(),
          type: 'deadlock',
          cycle: dl.thread_cycle,
          description,
          deadlock_details: {
            thread_cycle: dl.thread_cycle,
            thread_waiting_for_locks: waits.map(w => ({ thread_id: w.thread_id, lock_id: w.resource_id, resource_id: w.resource_id })),
            timestamp: dl.timestamp,
          },
        });
        console.log('[transformRawObject] Appended deadlock entry (object) at step', logs[logs.length-1].step);
      }
    }
  } catch (e) { console.warn('No terminal deadlock info or failed to parse', e); }

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
    
    return logsData; // Expecting { events, deadlock? }
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
      const out = transformRawObject(encodedStr);
      console.log('[processEncodedLog] non-string input; logs:', out.logs.length, 'graph steps:', out.graph_state.length);
      return out;
    }
    
    const decoded = decodeLogs(encodedStr);
    const out = transformRawObject(decoded);
    console.log('[processEncodedLog] decoded; logs:', out.logs.length, 'graph steps:', out.graph_state.length);
    // Ensure last event is visible in timeline
    if (out.logs.length > 0) {
      console.log('[processEncodedLog] last log type:', out.logs[out.logs.length-1].type);
    }
    return out;
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
    
    // Always parse upload as JSONL (one JSON object per line)
    if (typeof logText === 'string') {
      const lines = logText.split(/\r?\n/).filter(l => l.trim().length > 0);
        const events = [];
      let terminalDeadlock = null;
      for (const rawLine of lines) {
        const line = rawLine.trim().replace(/^\uFEFF/, '');
        try {
          const obj = JSON.parse(line);
          if (obj.event) {
            const { thread_id, lock_id, event, timestamp, parent_id } = obj;
              const eventCode = getEventCode(event);
            events.push([thread_id, lock_id, eventCode, timestamp, parent_id || 0]);
          } else if (obj.deadlock) {
            terminalDeadlock = obj.deadlock;
          }
        } catch (e) {
          console.error('Error parsing JSON line:', e, line);
        }
      }
      jsonData = { events, deadlock: terminalDeadlock };
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

    // If terminal deadlock exists in upload, append a deadlock step with rich description
    if (jsonData.deadlock) {
      const dl = jsonData.deadlock;
      const waits = (dl.thread_waiting_for_locks || []).map(w => Array.isArray(w) ? { thread_id: w[0], resource_id: w[1] } : w);
      const resourceTypeById = {};
      try {
        logs.forEach(e => {
          if (e && typeof e.type === 'string' && e.type.endsWith('_spawn') && e.resource_id) {
            const num = parseInt(e.resource_id);
            if (!isNaN(num)) resourceTypeById[num] = e.lock_type || 'mutex';
          }
        });
      } catch (_) {}
      const descLines = [];
      for (let i = 0; i < dl.thread_cycle.length; i++) {
        const threadId = dl.thread_cycle[i];
        const nextThread = dl.thread_cycle[(i + 1) % dl.thread_cycle.length];
        const wait = waits.find(w => w.thread_id === threadId);
        if (!wait) continue;
        const resId = parseInt(wait.resource_id);
        const rType = resourceTypeById[resId] || 'mutex';
        const rLabel = rType === 'rwlock' ? 'RwLock' : (rType === 'condvar' ? 'Condvar' : 'Mutex');
        descLines.push(`<span class="thread-id">Thread ${threadId}</span> is waiting for <span class="resource-id ${rType}">${rLabel} ${resId}</span> held by <span class="thread-id">Thread ${nextThread}</span>`);
      }
      const description = `<strong>DEADLOCK DETECTED:</strong><br>` + (descLines.length ? descLines.join('<br>') : '');

      logs.push({
        step: logs.length + 1,
        timestamp: Date.now(),
        type: 'deadlock',
        cycle: dl.thread_cycle,
        description,
        deadlock_details: {
          thread_cycle: dl.thread_cycle,
          thread_waiting_for_locks: waits.map(w => ({ thread_id: w.thread_id, lock_id: w.resource_id, resource_id: w.resource_id })),
          timestamp: dl.timestamp,
        },
      });
    }

    // Generate graph states based on logs (includes appended deadlock step)
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
    // Thread lifecycle
    case 'ThreadSpawn': return 0;
    case 'ThreadExit': return 1;
    
    // Mutex lifecycle 
    case 'MutexSpawn': return 2;
    case 'MutexExit': return 3;
    
    // RwLock lifecycle
    case 'RwSpawn': return 4;
    case 'RwExit': return 5;
    
    // Condvar lifecycle
    case 'CondvarSpawn': return 6;
    case 'CondvarExit': return 7;
    
    // Mutex interactions
    case 'MutexAttempt': return 10;
    case 'MutexAcquired': return 11;
    case 'MutexReleased': return 12;
    
    // RwLock interactions
    case 'RwReadAttempt': return 20;
    case 'RwReadAcquired': return 21;
    case 'RwReadReleased': return 22;
    case 'RwWriteAttempt': return 23;
    case 'RwWriteAcquired': return 24;
    case 'RwWriteReleased': return 25;
    
    // Condvar interactions
    case 'CondvarWaitBegin': return 30;
    case 'CondvarWaitEnd': return 31;
    case 'CondvarNotifyOne': return 32;
    case 'CondvarNotifyAll': return 33;
    
    // Legacy generic events for backward compatibility
    case 'Attempt': return 40;
    case 'Acquired': return 41;
    case 'Released': return 42;
    case 'Spawn': return 3; // Legacy mapping
    case 'Exit': return 4; // Legacy mapping
    
    default: return -1; // Unknown event
  }
}
