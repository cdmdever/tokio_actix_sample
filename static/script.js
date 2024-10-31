// Function to fetch UDP response from Rust server
async function fetchUdp() {
    try {
        const response = await fetch('https://127.0.0.1:8080/udp');
        const data = await response.json();
        console.log("UDP Response: ", data.msg);
        document.getElementById('udp-response').textContent = data.msg;
    } catch (error) {
        console.error("Error in UDP fetch: ", error);
    }
}

// Function to fetch TCP response from Rust server
async function fetchTcp() {
    try {
        const response = await fetch('https://127.0.0.1:8080/tcp');
        const data = await response.json();
        console.log("TCP Response: ", data.msg);
        document.getElementById('tcp-response').textContent = data.msg;
    } catch (error) {
        console.error("Error in TCP fetch: ", error);
    }
}

// Attach event listeners to buttons
document.getElementById('udp-button').addEventListener('click', fetchUdp);
document.getElementById('tcp-button').addEventListener('click', fetchTcp);
