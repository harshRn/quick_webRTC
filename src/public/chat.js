let divVideoChatLobby = document.getElementById("video-chat-lobby");
let divVideoChat = document.getElementById("video-chat-room");
let joinButton = document.getElementById("join");
let userVideo = document.getElementById("user-video");
let peerVideo = document.getElementById("peer-video");
let roomInput = document.getElementById("roomName");
let socket = null;
let sendMessage = null;
const queue = [];
let creator = null;
let cam_opened = null;
let userStream = null;
let rtcPeerConnection = null;

// Contains the stun server URL we will be using.
let iceServers = {
  iceServers: [
    { urls: "stun:stun.services.mozilla.com" },
    { urls: "stun:stun.l.google.com:19302" },
  ],
};

function createSocket(roomName) {
  // const ws = new WebSocket(`ws://192.168.1.17:9000/ws/${roomName}`);
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${protocol}//${window.location.host}/ws/abc`);

  function send(msg) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(msg);
    } else {
      queue.push(msg);
    }
  }

  ws.onopen = () => {
    console.log("WebSocket connected");
    // Send any queued messages
    while (queue.length) {
      ws.send(queue.shift());
    }
  };

  ws.onmessage = (e) => {
    // Handle your incoming messages here
    handleMessage(e.data, ws);
  };

  ws.onerror = (e) => {
    console.error("WebSocket error:", e);
  };

  ws.onclose = () => {
    console.log("WebSocket closed");
  };

  return { socket: ws, send };
}

function connect() {
  if (roomInput.value === "") {
    alert("Please enter a room name");
    return;
  }

  divVideoChatLobby.style.display = "none";
  const roomName = roomInput.value;

  const socketObj = createSocket(roomName);
  socket = socketObj.socket;
  sendMessage = socketObj.send;

  // Send your first message
  // sendMessage("first message from client side");
}

function parse(msg) {
  return typeof msg === "string" ? JSON.parse(msg) : msg;
}

// Parse incoming WebSocket message
function parseMessage(rawMessage) {
  try {
    const parsed = JSON.parse(rawMessage);

    // Handle plain string messages
    if (typeof parsed === "string") {
      return {
        type: parsed,
        data: null,
      };
    }

    // Handle object messages
    const messageType = Object.keys(parsed)[0];
    const messageData = parse(parsed[messageType]);

    return {
      type: messageType,
      data: messageData,
    };
  } catch (error) {
    console.error("Failed to parse message:", error);
    return null;
  }
}

// Route message to appropriate handler
function handleMessage(rawMessage, ws) {
  console.log("rawMessage :" + rawMessage);
  const message = parseMessage(rawMessage);

  if (!message) return;

  const handler = messageHandlers[message.type];
  if (handler) {
    handler(message.data, ws);
  }
}

function open_cam(created, socket) {
  creator = created;
  navigator.mediaDevices
    .getUserMedia({
      audio: true,
      // video: { width: 1280, height: 720 },

      video: {
        facingMode: "user", // Instead of width/height
      },
    })
    .then(function (stream) {
      /* use the stream */
      userStream = stream;
      divVideoChatLobby.style = "display:none";
      userVideo.srcObject = stream;
      // REMOVE
      userVideo.setAttribute("playsinline", "");

      userVideo.onloadedmetadata = function (e) {
        userVideo.play();
      };
      if (creator === false) {
        socket.send('"Ready"');
      }
    })
    .catch(function (err) {
      /* handle the error */
      console.log(err);
      alert("Couldn't Access User Media", err);
    });
  cam_opened = true;
}

function init_rtpc_connection() {
  if (creator) {
    rtcPeerConnection = new RTCPeerConnection(iceServers);
    rtcPeerConnection.onicecandidate = OnIceCandidateFunction;
    rtcPeerConnection.ontrack = OnTrackFunction;
    rtcPeerConnection.addTrack(userStream.getTracks()[0], userStream);
    rtcPeerConnection.addTrack(userStream.getTracks()[1], userStream);
    rtcPeerConnection
      .createOffer()
      .then((offer) => {
        rtcPeerConnection.setLocalDescription(offer);
        let sdp_message = {};
        sdp_message["Offer"] = offer;
        socket.send(JSON.stringify(sdp_message));
      })
      .catch((error) => {
        console.log(error);
      });
  }
}

// Triggered on receiving an offer from the person who created the room.
function check_offer(offer) {
  if (!creator) {
    rtcPeerConnection = new RTCPeerConnection(iceServers);
    rtcPeerConnection.onicecandidate = OnIceCandidateFunction;
    rtcPeerConnection.ontrack = OnTrackFunction;
    rtcPeerConnection.addTrack(userStream.getTracks()[0], userStream);
    rtcPeerConnection.addTrack(userStream.getTracks()[1], userStream);
    rtcPeerConnection.setRemoteDescription(offer);
    rtcPeerConnection
      .createAnswer()
      .then((answer) => {
        rtcPeerConnection.setLocalDescription(answer);
        let answer_message = {};
        answer_message["Answer"] = answer;
        // socket.emit("answer", answer, roomName);
        socket.send(JSON.stringify(answer_message));
      })
      .catch((error) => {
        console.log(error);
      });
  }
}

// Triggered on receiving an answer from the person who joined the room.
function check_answer(answer) {
  if (creator) {
    rtcPeerConnection.setRemoteDescription(answer);
  }
}

// Implementing the OnIceCandidateFunction which is part of the RTCPeerConnection Interface.

function OnIceCandidateFunction(event) {
  if (event.candidate) {
    // socket.emit("candidate", event.candidate, roomName);
    let candidate_msg = {};
    let candidate = JSON.parse(JSON.stringify(event.candidate));
    candidate.creator = creator;
    candidate_msg["Candidate"] = candidate;
    console.log("sending : " + candidate_msg);
    socket.send(JSON.stringify(candidate_msg));
  }
}

// // Implementing the OnTrackFunction which is part of the RTCPeerConnection Interface.

function OnTrackFunction(event) {
  peerVideo.srcObject = event.streams[0];
  peerVideo.onloadedmetadata = function (_e) {
    peerVideo.play();
  };
}

function check_candidate(candidate) {
  if (candidate.creator === creator) {
    // no way to distinguish when broadcasting. re-work in case of more than 2 callers
    return;
  }
  let icecandidate = new RTCIceCandidate(candidate);
  rtcPeerConnection.addIceCandidate(icecandidate);
}

// Message handlers
const messageHandlers = {
  Alert: (data, socket) => {
    alert(`${data.connection_result}: IP ${data.ip} in room ${data.room}`);
  },
  Text: (data, socket) => {
    console.log(
      `${data.connection_result}: IP ${data.ip} in room ${data.room}`
    );
    if (cam_opened === null) {
      if (data.connection_result === "Created") {
        open_cam(true, socket);
      } else if (data.connection_result === "Joined") {
        open_cam(false, socket);
      }
    }
  },
  Ready: (data) => {
    init_rtpc_connection();
  },
  Offer: (data) => {
    check_offer(data);
  },
  Answer: (data) => {
    check_answer(data);
  },
  Candidate: (data) => {
    check_candidate(data);
  },
};

joinButton.addEventListener("click", connect);
// sendMessage("first message from client side");
