// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use dicom::object::{FileDicomObject, InMemDicomObject, Tag};
use dicom::{object::open_file, pixeldata::PixelDecoder};
use dicom::dictionary_std::tags;
use ndarray::{s, Array, ArrayBase, Axis, Dim, OwnedRepr};
use dicom::pixeldata::image::GrayImage;
use ndarray_stats::QuantileExt;
use tauri::Manager;
use std::collections::HashMap;
use std::fs;
use std::cmp::{max, min};
use std;
// use std::cmp;

// type
type DcmObj = dicom::object::FileDicomObject<dicom::object::InMemDicomObject>;
type U16Array = ArrayBase<OwnedRepr<u16>, Dim<[usize; 2]>>;
type I32Array = ArrayBase<OwnedRepr<i32>, Dim<[usize; 2]>>;
type Obj = FileDicomObject<InMemDicomObject>;

// constant
const PI: f64 = 3.14159;

#[tauri::command]
fn processing(file_path: String, save_path: String) -> (HashMap<String, Vec<f32>>, Vec<u128>, Vec<String>) {
    match open_dcm_file(file_path) {
        Some(obj) => {
            let pixel_data: dicom::pixeldata::DecodedPixelData<'_> = obj.decode_pixel_data().unwrap();
            let arr=  pixel_data.to_ndarray::<u16>().unwrap().slice(s![0, .., .., 0]).to_owned();

            // details
            let hospital = get_detail(&obj, tags::INSTITUTION_NAME);
            let manufacturer = get_detail(&obj, tags::MANUFACTURER);
            let acquisition_date = get_detail(&obj, tags::ACQUISITION_DATE);
            let detector_type = get_detail(&obj, tags::DETECTOR_TYPE);
            let detector_id = get_detail(&obj, tags::DETECTOR_ID);
            let modality = get_detail(&obj, tags::MODALITY);
            let machine = format!("{} [{}]", manufacturer, modality);
            let address = get_detail(&obj, tags::INSTITUTION_ADDRESS);
            let patient_id = get_detail(&obj, tags::PATIENT_ID);
            let spatial_resolution = get_detail(&obj, tags::SPATIAL_RESOLUTION);
            let mut pixel_size = "".to_string();
            if spatial_resolution != "".to_string() {
                pixel_size = format!("{}x{} mm", spatial_resolution, spatial_resolution);
            }
            let rows_ = get_detail(&obj, tags::ROWS);
            let cols_ = get_detail(&obj, tags::COLUMNS);
            let mut matrix_size = format!("");
            if (rows_ != "".to_string()) && (cols_ != "".to_string()) {
                matrix_size = format!("{}x{}", rows_, cols_);
            } 
            let bit_depth = get_detail(&obj, tags::BITS_STORED);
            let details = vec![hospital, machine, address, acquisition_date, detector_type, detector_id, patient_id, pixel_size, matrix_size, bit_depth];
            // find MTF bar
            let (arr, need_inv) = find_mtf_bar(arr);
            // // done
            // let arr: U16Array = arr.slice(s![crop[0]..crop[1], crop[2]..crop[3]]).to_owned();

            // // just in this case
            // let arr_rotated: ArrayBase<OwnedRepr<u16>, Dim<[usize; 2]>> = rotate_array(PI, arr);
            // save_to_image(arr_rotated.clone(), save_path);
            // let theta_r = find_theta(arr_rotated.clone());
            // let arr = rotate_array(theta_r, arr_rotated);
            // // focus one line to find linepairs position
            // let (focus, linepairs, one_line) = linepairs_pos(arr);
            // let res = calculate_details(focus, linepairs);
            // return (res, one_line, details);
            return (HashMap::new(), vec![], vec![]);
        }, 
        None => {
            println!("NOT FOUND!");
            return (HashMap::new(), vec![], vec![]);
        }
    }
}

fn open_dcm_file(file_path: String) -> Option<DcmObj> {
    match open_file(file_path) {
        Ok(obj) => {
            return Some(obj);
        }, 
        Err(_) => {
            return None;
        }
    }
}

fn find_mtf_bar(mut arr: U16Array) -> (U16Array, bool) {
    let shape = arr.shape();
    let h = shape[0];
    let w = shape[1];
    let mut ecrop_h = i32::MAX; // in case crop2000x2000
    let mut ecrop_w = i32::MAX; 
    // assume crop if size > 2,000 (not crop yet)
    let p = 0.24;
    if h*w > 2000*2000 {
        let crop = [
            (p*(h as f32)) as i32,
            ((1.0-p)*(h as f32)) as i32,
            (p*(w as f32)) as i32,
            ((1.0-p)*(w as f32)) as i32,
            ];
            arr = arr.slice(s![crop[0]..crop[1], crop[2]..crop[3]]).to_owned();
            ecrop_h = arr.nrows() as i32;
            ecrop_w = arr.ncols() as i32;
        };
    // edge crop
    let ecrop = [
        (0.05*(h as f32)) as i32,
        (0.95*(h as f32)) as i32,
        (0.05*(w as f32)) as i32,
        (0.95*(w as f32)) as i32,
    ];
    arr = arr.slice(s![ecrop[0]..min(ecrop[1], ecrop_h), ecrop[2]..min(ecrop[3], ecrop_w)]).to_owned();
    let shape = arr.shape();
    let h = shape[0];
    let w = shape[1];
    // find MTF bar
    let p = 0.33;
    let p_mean_arr = arr.slice(s![
        (p*(h as f32)) as i32..((1.0-p)*(h as f32)) as i32,
        (p*(w as f32)) as i32..((1.0-p)*(w as f32)) as i32
        ]).to_owned();
    let p_mean_arr = p_mean_arr.mapv(|x| x as u128);
    let p_mean: u16 = p_mean_arr.mean().unwrap() as u16;
    let parr = arr.iter()
        .map(|&x| if x > p_mean { 1 } else { 0 })
        .collect::<Vec<_>>();
    let parr = Array::from_shape_vec((h, w), parr).unwrap();
    // width finding
    let vc = [
        (0.1 * h as f32) as i32,
        (0.5 * h as f32) as i32,
    ];
    let varr = parr.slice(s![
        vc[0]..vc[1], 0..w  
    ]).to_owned();
    let (w_min, w_max) = find_edge_col(varr);
    // height finding
    let hc = [
        (0.1 * h as f32) as i32,
        (0.5 * h as f32) as i32,
    ];
    let harr = parr.slice(s![
        0..h, hc[0]..hc[1]
    ]).to_owned();
    let (h_min, h_max, start_val) = find_edge_row(harr);

    // processing crop, inv, rotate to horizontal
    let need_inv = start_val == 1;
    let l1 = h_max-h_min;
    let l2 = w_max-w_min;
    let need_rotate = l1>l2;
    let add_crop = if l1 > l2 {
        (0.1 * l1 as f32) as i32 // Cast to f32 for multiplication, then back to u16
    } else {
        (0.1 * l2 as f32) as i32
    };
    let h_min = max(0, h_min-add_crop);
    let h_max = min(h_max+add_crop, h as i32);
    let w_min = max(0, w_min-add_crop);
    let w_max = min(w_max+add_crop, w as i32);
    let mut arr = arr.slice(s![
        h_min..h_max, w_min..w_max
    ]).to_owned();
    if need_inv {
        let max_val = arr.max().unwrap();
        arr = arr.mapv(|x| (max_val-x) as u16);
    }
    if need_rotate {
        let s = arr.shape();
        let nrows = s[0];
        let ncols = s[1];
        let mut arr_vec = vec![];
        for r in 0..nrows {
            let mut row_vec = vec![];
            for c in 0..ncols {
                row_vec.push(arr[(r, c)] as i32);
            }
            arr_vec.push(row_vec);
        }
        let arr_vec_rotated = rotate_matrix_cw(arr_vec);
        arr = Array::from_shape_vec((ncols, nrows), arr_vec_rotated.concat()).unwrap();
        dbg!(arr.shape());
    }
    (arr, need_inv)
}

fn get_detail(obj: &Obj, tags: Tag) -> String {
    match obj.element(tags) {
            Ok(obj) => {
                return obj.to_str().unwrap().to_string();
            }, 
            Err(_) => {
                return "".to_string();
            }
        }
    }

fn find_edge_col(arr: I32Array) -> (i32, i32) {
    let shape = arr.shape();
    let nrows = shape[0];
    let ncols = shape[1];
    let mut none_count = 0;
    let mut appended = false;
    let nont_ts = 10;
    let mut start_none_count = 0;
    let start_val = arr[(
        (0.2 * nrows as f32) as usize,
        (0.2 * ncols as f32) as usize
    )];
    let mut edges_pos = vec![];
    for c in 0..ncols {
        let mut is_c = false;
        for r in 0..nrows {
            let val = arr[(r, c)];
            if val != start_val {
                edges_pos.push(c);
                is_c = true;
                appended = true;
                none_count = 0;
                break;
            }
        }
        if !is_c {
            none_count += 1;
            start_none_count += 1;
            edges_pos.push(0);
            if (none_count >= nont_ts) && appended {
                break;
            }
        }
    }
    let w_min = edges_pos[start_none_count+1-nont_ts] as i32;
    let w_max = edges_pos[edges_pos.len()-nont_ts-1] as i32;
    // check is correct
    if w_max-w_min < (0.15*ncols as f32) as i32 {
        let w_crop = w_max-w_min + 5;
        let (w_min, w_max) = find_edge_col(
            arr.slice(s![
                0..nrows, w_crop..ncols as i32
            ]).to_owned()
        );
        let w_min = w_min + w_crop;
        let w_max = w_max + w_crop;
        return  (w_min, w_max);
    }
    (w_min, w_max)
}

fn find_edge_row(arr: I32Array) -> (i32, i32, i32) {
    let shape = arr.shape();
    let nrows = shape[0];
    let ncols = shape[1];
    let mut none_count = 0;
    let mut appended = false;
    let nont_ts = 10;
    let mut start_none_count = 0;
    let start_val = arr[(
        (0.2 * nrows as f32) as usize,
        (0.2 * ncols as f32) as usize
    )];
    let mut edges_pos = vec![];
    for r in 0..nrows {
        let mut is_r = false;
        for c in 0..ncols {
            let val = arr[(r, c)];
            if val != start_val {
                edges_pos.push(r);
                is_r = true;
                appended = true;
                none_count = 0;
                break;
            }
        }
        if !is_r {
            none_count += 1;
            start_none_count += 1;
            edges_pos.push(0);
            if (none_count >= nont_ts) && appended {
                break;
            }
        }
    }
    let h_min = edges_pos[start_none_count+1-nont_ts] as i32;
    let h_max = edges_pos[edges_pos.len()-nont_ts-1] as i32;
    // check is correct
    if h_max-h_min < (0.15*nrows as f32) as i32 {
        let h_crop = h_max-h_min + 5;
        let (h_min, h_max, start_val) = find_edge_row(
            arr.slice(s![
                h_crop..nrows as i32, 0..ncols
            ]).to_owned()
        );
        let h_min = h_min + h_crop;
        let h_max = h_max + h_crop;
        return  (h_min, h_max, start_val);
    }
    (h_min, h_max, start_val)
}

fn rotate_matrix_cw(matrix: Vec<Vec<i32>>) -> Vec<Vec<u16>> {
    let rows = matrix.len();
    let cols = matrix[0].len();

    // Transpose the matrix
    let mut transposed = vec![vec![0; rows]; cols];
    for i in 0..rows {
        for j in 0..cols {
            transposed[j][i] = matrix[i][j] as u16;
        }
    }

    // Reverse the order of rows
    for row in transposed.iter_mut() {
        row.reverse();
    }

    transposed
}

fn rotate_array(theta_r: f64, array: U16Array) -> U16Array{
    // rotate array CW by theta in radius 
    let h = array.nrows();
    let w = array.ncols();
    let mut rotated = ndarray::Array::zeros((h as usize, w as usize));
    let center_x = w as f64 / 2.;
    let center_y = h as f64 / 2.;   
    
    for i in 0..h {
        for j in 0..w {            
            let x = j as f64 - center_x;
            let y = i as f64 - center_y;
            
            let new_x = x * theta_r.cos() - y * theta_r.sin() + center_x;
            let new_y = x * theta_r.sin() + y * theta_r.cos() + center_y;

            let new_i = new_y.round() as usize;
            let new_j = new_x.round() as usize;
            
            if new_i < h && new_j < w {
                rotated[(new_i, new_j)] = array[(i, j)];
            }
        }
    }

    rotated
}

fn save_to_image(array: U16Array, save_path: String) {
    // save array to image
    let h = array.nrows();
    let w = array.ncols();
    let u8_gray: Vec<u8> = convert_to_u8(array.clone().into_raw_vec(), array.len());
    let img = array_to_image(u8_gray, h as u32, w as u32);
    img.save(save_path).unwrap();
}

fn convert_to_u8(pixel_vec: Vec<u16>, size: usize) -> Vec<u8> {
    let mut res: Vec<u8> = Vec::with_capacity(size);
    let max_value = *pixel_vec.iter().max().unwrap() as f32;
    for &value in &pixel_vec {
        let u8_val = ((value as f32 / max_value)* 255.) as u8;
        res.push(u8_val);
    }
    res
}

fn array_to_image(pixel_vec: Vec<u8>, h: u32, w: u32) -> GrayImage {
    GrayImage::from_raw(w, h, pixel_vec).unwrap()
}

fn find_theta(arr: U16Array) -> f64 {
    // find theta for rotated to straight line
    let h = arr.nrows() as i32;
    let w = arr.ncols() as i32;
    // crop ratio
    let hp = (0.28*(h as f32)) as i32;
    let wp = (0.03*(w as f32)) as i32;
    // crop right and left 
    // left
    let focus_l = arr.slice(s![
        h-(2*hp)..(h as f32 * 0.95) as i32, wp*9..wp*11
    ]).to_owned();
    
    let arg_diffs = arg_diffs_col(focus_l);
    let y1 = find_most_common(arg_diffs);
    // right
    let focus_r = arr.slice(s![
        h-(2*hp)..(h as f32 * 0.95) as i32, w-(wp*11)..w-(wp*9)
    ]).to_owned();
    let arg_diffs = arg_diffs_col(focus_r);
    let y2 = find_most_common(arg_diffs);

    // find theta
    let a = y2 - y1;
    let ratio = a as f64/w as f64;
    let theta_r = ratio.atan();
    -theta_r // negative because fn rotated CW
}

fn arg_diffs_col(arr: U16Array) -> Vec<u16> {
    // find positions most different value by column
    let nrows = arr.nrows();
    let ncols = arr.ncols();
    let mut max_diff;
    let mut argmax_diff;
    let mut arg_diffs = vec![];
    for c in 0..ncols {
        max_diff = 0;
        argmax_diff = 0;
        for r in 0..nrows {
            if r+1 < nrows {
                let cur_val = arr[(r, c)] as i32;
                let next_val = arr[(r+1, c)] as i32;
                let diff = i32::abs(cur_val - next_val);
                if diff > max_diff {
                    max_diff = diff;
                    argmax_diff = r;
                }
            }
        }
        arg_diffs.push(argmax_diff as u16);
    }
    arg_diffs
}

fn find_most_common(array: Vec<u16>) -> i32 {
    // find most common value in vector 
    // just hashmap
    let mut counts: HashMap<u16, u16> = HashMap::new();
    for n in &array {
        let count = counts.entry(*n).or_insert(0);
        *count += 1;
    }
    // then find maximun by value(count) but return key
    let mut max_key = None;
    let mut max_val = std::u16::MIN;
    for (k, v) in counts {
        if v > max_val {
            max_key = Some(k);
            max_val = v;
        }
    }
    max_key.unwrap() as i32
}

fn linepairs_pos(arr: U16Array) -> (U16Array, Vec<(usize, usize)>, Vec<u128>) {
    // find linpairs position 
    let h = arr.nrows() as i32;
    let w = arr.ncols() as i32;
    let hp = (0.11*(h as f32)) as i32;
    let wp = (0.10*(w as f32)) as i32;
    // crop 
    let real_focus = arr.slice(s![
        (h/2)-hp..(h/2)+hp, (wp as f32 * 1.5) as i32..w-((wp as f32 * 1.2) as i32)
    ]).to_owned();
    // change type to prevent add overflow
    let focus = real_focus.mapv(|x| x as u128);
    let one_line = focus.mean_axis(Axis(0)).unwrap().into_raw_vec(); // 0 is axis by col
    // find diff vals each pixel
    let mut diff_vals: Vec<i128> = vec![];
    let total = one_line.len();
    for idx in 0..total {
        if idx + 1 < total {
            let cur_val = one_line[idx] as i32;
            let next_val = one_line[idx+1] as i32;
            let diff = i32::abs(cur_val - next_val);
            diff_vals.push(diff as i128);
        }
    }
    // make looks easier
    let mut new_ts: Vec<u8> = vec![]; 
    let sum_: i128 = diff_vals.iter().sum();
    let threshold = (sum_ as f64 / total as f64) as i128;
    let mut new_val;
    for val in diff_vals {
        if val < threshold {
            new_val = 0;
        } else {
            new_val = 1;
        }
        new_ts.push(new_val)
    }
    // find real zero positions
    let mut zero_positions: Vec<usize> = vec![0];
    let mut is_start = false;
    let mut is_start_zero = true;
    let mut start_zero_pos = 0;
    let total = new_ts.len();
    let cut_count = (total as f32 * 0.015) as u16; // how many zero that count to real zero
    let mut count = 0;
    for (idx, value) in new_ts.iter().enumerate() {
        if idx+1 == total {
            zero_positions.push(start_zero_pos);
            zero_positions.push(idx);
        }
        if *value == 1 {
            if count >= cut_count {
                zero_positions.push(start_zero_pos);
                zero_positions.push(idx);
            }
            is_start = true;
            is_start_zero = true;
            count = 0;
        }
        if is_start {
            if *value == 0 {
                if is_start_zero {
                    start_zero_pos = idx;
                    is_start_zero = false;
                }
                count += 1;
            }
        } 
    }
    // linepairs positions
    let trim = (total as f32 * 0.004) as usize;
    let linepairs: Vec<(usize, usize)> = zero_positions
        .iter()
        .enumerate()
        .filter(|(idx, _)| idx % 2 == 0 && idx+1 < zero_positions.len())  // step by 2 and prevent over index
        .map(|(idx, &pos)| (pos + trim, zero_positions[idx + 1] - trim))
        .collect();
    (real_focus, linepairs, one_line)
}

fn calculate_details(focus: U16Array, linepairs: Vec<(usize, usize)>) -> HashMap<String, Vec<f32>> {
    // calculate details value in MTF linepairs
    let focus = focus.mapv(|x| x as i128);
    let min_val0 = *focus.slice(s![
        .., linepairs[0].0..linepairs[0].1
    ]).min().unwrap() as f32;
    let max_val0 = *focus.slice(s![
        .., linepairs[0].1..linepairs[1].0
    ]).max().unwrap() as f32;
    let contrast0 = (max_val0 - min_val0) as f32; // some bad precision error

    // result
    let mut res: HashMap<String, Vec<f32>> = HashMap::new();
    res.insert("Linepair".to_string(), vec![0.0]);
    res.insert("Max".to_string(), vec![max_val0]);
    res.insert("Min".to_string(), vec![min_val0]);
    res.insert("Contrast".to_string(), vec![contrast0]);
    res.insert("Modulation".to_string(), vec![100.0]);
    res.insert("start".to_string(), vec![0.0]);
    res.insert("end".to_string(), vec![0.0]);

    // skip first because already find value
    for idx in 1..linepairs.len() {
        let (start, end) = linepairs[idx];
        let linepair = focus.slice(s![
            .., start..end
        ]).to_owned();
        let mean_val_col = linepair.mean_axis(Axis(0)).unwrap();
        // let mut sorted_val = mean_val_col.into_raw_vec();
        // sorted_val.sort(); //  to seperate max and min vals
        // let mid_pos = cmp::max(cmp::min(
            // (end-start)/2, ((end-start) as f32 * 0.3) as usize
        // ), 1); // prevent mid_pos is 0
        // min vals
        // mean_min_vals = round(np.mean(sorted_val[: mid_pos]))
        // let sum_min_vals: i128 = sorted_val[0..mid_pos].iter().sum();
        // let mean_min_vals: f32 = sum_min_vals as f32 / mid_pos as f32;
        // max vals
        // mean_max_vals = round(np.mean(sorted_val[-mid_pos: ]))
        // let sum_max_vals: i128 = sorted_val[(sorted_val.len()-mid_pos)..sorted_val.len()].iter().sum();
        // let mean_max_vals: f32 = sum_max_vals as f32 / sorted_val[(sorted_val.len()-mid_pos)..sorted_val.len()].len() as f32;
        let min_vals = *mean_val_col.min().unwrap();
        let max_vals = *mean_val_col.max().unwrap();
        // contrast and modulation
        let contrast = (max_vals - min_vals) as f32;
        let modulation = contrast*100.0/contrast0;
        
        // res.get_mut("Linepair").unwrap().push(idx as f32);
        res.get_mut("Max").unwrap().push(max_vals as f32);
        res.get_mut("Min").unwrap().push(min_vals as f32);
        res.get_mut("Contrast").unwrap().push(contrast);
        res.get_mut("Modulation").unwrap().push(modulation);
        res.get_mut("start").unwrap().push(start as f32);
        res.get_mut("end").unwrap().push(end as f32);
    }
    res
}

// splashscreen
#[tauri::command]
fn close_splashscreen(window: tauri::window::Window) {
    if let Some(splashscreen) = window.get_window("splashscreen") {
        splashscreen.close().unwrap();
    }
    window.get_window("home").unwrap().show().unwrap();
} 

// home -> processing
#[tauri::command]
fn home2processing(window: tauri::window::Window) {
  if let Some(splashscreen) = window.get_window("home") {
        splashscreen.hide().unwrap();
    }
    window.get_window("main").unwrap().show().unwrap();  
}

//  processing -> hone
#[tauri::command]
fn processing2home(window: tauri::window::Window) {
    if let Some(process) = window.get_window("main") {
        process.hide().unwrap();
    }
    window.get_window("home").unwrap().show().unwrap();
}

#[tauri::command]
fn write_file(content: String, save_path: String) {
    fs::write(save_path, content).unwrap();
}

#[tauri::command]
fn read_file(file_path: String) -> String {
    let content = fs::read_to_string(file_path).unwrap();
    content
}

#[tauri::command]
fn write_csv(save_path: String, content: String) {
    let content = content.replace("/n", "\n");
    fs::write(save_path, content).unwrap();
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![processing, close_splashscreen, home2processing, processing2home, write_file, read_file, write_csv])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
