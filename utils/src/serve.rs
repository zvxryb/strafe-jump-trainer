/*
 * Copyright 2019 Michael Lodato <zvxryb@gmail.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

extern crate warp;

use warp::Filter;

use std::path::PathBuf;

fn main() {
    let mut project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    project_dir.pop();

    let mut index_path = project_dir.clone();
    index_path.push("static");
    index_path.push("index.html");

    let mut pkg_dir = project_dir.clone();
    pkg_dir.push("pkg");

    println!("{}", index_path.to_str().unwrap());
    println!("{}", pkg_dir.to_str().unwrap());

    let index = warp::get2()
        .and(warp::path::end())
        .and(warp::fs::file(index_path));
    let pkg = warp::path("pkg")
        .and(warp::fs::dir(pkg_dir));
    
    let routes = index.or(pkg);

    warp::serve(routes).run(([127, 0, 0, 1], 8080));
}