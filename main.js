const { invoke } = window.__TAURI__.tauri;
const { tempdir } = window.__TAURI__.os;
const { convertFileSrc } = window.__TAURI__.tauri;
const { appWindow } = window.__TAURI__.window;
const { save } = window.__TAURI__.dialog;

// let openFile;
let fileName;
let mainContainer;
let newFile;
let mtfImage;
let tableDetails;
let displayTab;
let classNameTab;
let tabsBar;
let content;
let loader;
let mainElm;
let compareCsv = "";
let elements = {"fileNames": [], "modulations": []};
const linepairs = [
    0.0, 1.0, 1.11, 1.23, 1.37, 1.52, 1.69, 1.88, 2.09,
    2.32, 2.58, 2.87, 3.19, 3.54, 3.93, 4.37, 4.86
]

window.tabShow = function (idx) {
    for (let i = 0; i <= elements["modulations"].length; i++) {
        document.getElementById(`tab${i}`).classList.remove("activeBtn");
        document.getElementById(`container${i}`).style.display = "none";
    }
    const container = document.getElementById(`container${idx}`);
    const tabs = document.getElementById(`tab${idx}`);
    tabs.classList.add("activeBtn");
    container.style.display = "flex";
}

window.saveCsv = async function (savePath, contentCsv) {
    const filePath = await save({
    filters: [{
        name: 'csv',
        extensions: ['csv']
    }],
    defaultPath: savePath
    });
    await invoke("write_csv", {savePath: filePath, content: contentCsv});
}

async function process(content) {
    let details = content.split("\n");
    let userName = details[0];
    let room = details[1];
    let filePaths = details.slice(2);
    const tempDir = await tempdir();
    if (filePaths) {
        for (let idx=0; idx<filePaths.length; idx++) {
            let filePath = filePaths[idx];
            let imagePath = `${tempDir}mtf-image000${idx}.jpg`;
            let texts = filePath.split("\\");
            let fileName = texts[texts.length-1];
            let savePath = `MTF_${fileName}`;
            elements["fileNames"].push(fileName);
            const column_name = ["Linepair/mm", "Max", "Min", "Contrast", "Modulation"];
            const res = await invoke("processing", {filePath: filePath, savePath: imagePath});
            if (idx == 0) {
                displayTab = "flex";
                classNameTab = "activeBtn"
            } else {
                displayTab = "none";
                classNameTab = "null";
            }
            
            if (filePaths.length > 1) {
                tabsBar.innerHTML += `<button id="tab${idx}" class=${classNameTab} onclick="tabShow(${idx})">${fileName}</button>`
            }

            // 
            let info = res[2];

            // plot0
            const mtf = res[1];
            let mtf_x = []
            for (let i=0; i< mtf.length; i++) {mtf_x.push(i)};
            
            // plot1
            const details = res[0];
            const contrast = details["Contrast"];
            const max_ = details["Max"];
            const min_ = details["Min"];
            const modulation = details["Modulation"]; 
            const start = details["start"]; 
            const end = details["end"];
            const csvInfo = [linepairs, max_, min_, contrast, modulation];
           
            // compare csv
            compareCsv += `${fileName},,,,/n`;

            // to .csv
            let contentCsv = column_name[0];
            for (let name of column_name.slice(1)) {
                contentCsv += `,${name}`
            }
            contentCsv += "/n";

            for (let idx=0; idx<linepairs.length; idx++) {
                for (let info of csvInfo) {
                    if (info == linepairs) {
                        contentCsv += `${info[idx]}`;
                    } else if (info == modulation){
                        contentCsv += `,${info[idx].toFixed(2)}`;
                    } else {
                        contentCsv += `,${info[idx]}`;
                    }
                }
                contentCsv += "/n";
            };
            compareCsv += contentCsv;
            compareCsv += "/n";

            mainContainer.innerHTML += `
                <div class="container" id="container${idx}" style="display: ${displayTab};">
                    <div class="left">
                        <span>
                            <p>Name : ${userName}</p>
                            <p>Room : ${room}</p>
                            <p id="hospital${idx}">Hospital : ${info[0]}</p>
                        </span>  
                        <table id="tableDetails${idx}"></table>
                    </div>
                    <div class="mid">
                        <img src="" id="mtfImage${idx}" style="width: 500px;">
                        <div id="mtf-plot0${idx}" style="width: 600px; height: 400px"></div>
                    </div>
                    <div class="right">
                        <div class="top-right">
                            <span>
                                <h3>INFORMATION</h3>
                                <p>File Name : ${fileName}</p>
                                <p>Manufacturer : ${info[1]}</p>
                                <p>Institution Address : ${info[2]}</p>
                                <p>Acquisition Date : ${info[3]}</p>
                                <p>Detector Type : ${info[4]}</p>
                                <p>Detector ID : ${info[5]}</p>
                                <p>Patient ID : ${info[6]}</p>
                                <p>Pixel Size : ${info[7]}<sup>2</sup></p>
                                <p>Matrix Size : ${info[8]}</p>
                                <p>Bit Depth : ${info[9]}</p>
                            </span>
                            <button id="export" onclick="saveCsv('${savePath}', '${contentCsv}')">Export</button>
                        </div>  
                        <div id="mtf-plot1${idx}" style="width: 600px; height: 400px"></div>
                    </div>
                </div>
            `

            mtfImage = document.querySelector(`#mtfImage${idx}`);
            tableDetails = document.querySelector(`#tableDetails${idx}`);

            // add for conparison
            elements["modulations"].push(modulation);

            mtfImage.src = convertFileSrc(imagePath);
            // fileName.textContent = name;

            // bind:plot0
            const mtf_line = {
                x: mtf_x,
                y: mtf,
                mode: "lines",
                name: "pixel value",
                line: {
                    color: "rgb(0, 0, 0)",
                    width: 1
                }
            };
            const layout0 = {
                showlegend: false,
                xaxis: {
                    title: "Position"
                },
                yaxis: {
                    title: "Pixel value"
                },
                dragmode: false,
                hovermode: false,
            };
            
            let data0 = [mtf_line];
            // max
            for (let idx=1; idx<contrast.length; idx++) {
                data0.push(
                    {
                        x: [start[idx], end[idx]],
                        y: [max_[idx], max_[idx]],
                        mode: "lines",
                        name: "maximum",
                        line: {
                            color: "rgb(255, 0, 0)",
                            width: 2,
                        }
                    }
                );
            };
            // min
            for (let idx=1; idx<contrast.length; idx++) {
                data0.push(
                    {
                        x: [start[idx], end[idx]],
                        y: [min_[idx], min_[idx]],
                        mode: "lines",
                        name: "minimum",
                        line: {
                            color: "rgb(0, 0, 255)",
                            width: 2
                        }
                    }
                )
            }

            // bind:plot1
            const modulation_plot = {
                x: linepairs,
                y: modulation,
                mode: "lines+markers",
            }
            const layout1 = {
                title: "Modulation Transfer Function (MTF)",
                xaxis: {
                    "title": "Linepair/mm"
                },
                yaxis: {
                    "title": "Modulation(%)"
                },
                dragmode: false,
                hovermode: false,
            }

            // table
            let tableHtml = "<tr>";
            for (name of column_name) {
                tableHtml += `<th>${name}</th>`
            }

            for (let idx=0; idx<contrast.length; idx++) {
                tableHtml += "<tr>";
                for (name of column_name) {
                    if (name != "Modulation") {
                        if (name == "Linepair/mm") {
                            tableHtml += `<td>${linepairs[idx]}</td>`
                        } else {
                            tableHtml += `<td>${details[name][idx].toFixed(0)}</td>`
                        }
                    } else {
                        tableHtml += `<td>${details[name][idx].toFixed(2)}</td>`
                    }
                }
                tableHtml += "</tr>";
            }

            
            tableHtml += "</tr>"
            tableDetails.innerHTML = tableHtml;

            Plotly.newPlot(`mtf-plot0${idx}`, data0, layout0);
            Plotly.newPlot(`mtf-plot1${idx}`, [modulation_plot], layout1);
        }
        // comparison 
        if (elements["modulations"].length > 1) {
            // current date
            let currentDate = new Date();
            let year = currentDate.getFullYear();
            let month = (currentDate.getMonth() + 1).toString().padStart(2, '0');
            let day = currentDate.getDate().toString().padStart(2, '0');
            let date = `${year}-${month}-${day}`;

            tabsBar.innerHTML += `<button id="tab${elements["modulations"].length}" onclick="tabShow(${elements["modulations"].length})">Comparison</button>`
            mainContainer.innerHTML += `
            <div class="container-compare" id="container${elements['modulations'].length}" style="display: none">
                <div id="mtf-plot1compare"></div>
                <div class="info-compare">
                    <span>
                        <h3>INFORMATION</h3>
                        <p>Name : ${userName}</p>
                        <p>Room : ${room}</p>
                        <p>Processing Date : ${date}</p>
                    </span>
                    <button id="export" onclick="saveCsv('MTF_Compare', '${compareCsv}')">Export</button>
                </div>
            </div>
        `
            // bind:plot1
            let data = [];
            for (let idx=0; idx<elements["modulations"].length; idx++) {
                data.push({
                    x: linepairs,
                    y: elements["modulations"][idx],
                    mode: "lines+markers",
                    name: elements["fileNames"][idx],
                })
            };

            const layout1 = {
                title: "Modulation Transfer Function (MTF)",
                xaxis: {
                    "title": "Linepair/mm"
                },
                yaxis: {
                    "title": "Modulation(%)"
                }
            }
            Plotly.newPlot(`mtf-plot1compare`, data, layout1);
        };
        showLoad(false);
    };
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function checkContent() {
    // load new .txt from home.html
    while (true) {
        try {
            const visible = await appWindow.isVisible();
            await sleep(1000);
            if (visible) {
                let tempDir = await tempdir();
                let path = `${tempDir}MTFhomedetails.txt`;
                let content = await invoke("read_file", {filePath: path});
                process(content);
                break;
            }
        } catch {
            console.log("Not found");
        };
    };
};

function showLoad(show) {
    if (show) {
        mainElm.style.display = "none";
        loader.style.display = "block";
    } else {
        loader.style.display = "none";
        mainElm.style.display = "block";
    }
}

window.addEventListener("DOMContentLoaded", async () => {
    newFile = document.querySelector("#newFile");
    mainContainer = document.querySelector(".main-container");
    tabsBar = document.querySelector("#tabsBar");
    loader = document.querySelector(".load-container");
    mainElm = document.querySelector(".main-element");
    checkContent();

    newFile.addEventListener("click", async () => {
        await invoke("processing2home");
        location.reload() // refresh variable
        checkContent();
    })    
});
