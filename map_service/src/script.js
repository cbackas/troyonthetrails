import Feature from 'ol/Feature.js';
import Map from 'ol/Map.js';
import { Vector as VectorSource, OSM as OSMSource } from 'ol/source';
import {fromLonLat} from 'ol/proj.js';
import View from 'ol/View.js';
import {
  Stroke,
  Style,
} from 'ol/style.js';
import {Tile as TileLayer, Vector as VectorLayer} from 'ol/layer.js';
import * as polylib from '@mapbox/polyline';
import { LineString } from 'ol/geom';

// thunderforest API KEY
const key = '{{ thunderforest_api_key }}';

const urlParams = new URLSearchParams(window.location.search);

// if no polyline is provided, exit
if (urlParams.get('polyline') == null) {
  process.exit(0);
}

const polyline = decodeURIComponent(urlParams.get('polyline')).replace(/\\\\/g , '\\')
let coords = polylib.decode(polyline);

function findMedian(values) {
  values.sort((a, b) => a - b);
  const mid = Math.floor(values.length / 2);

  if (values.length % 2 === 0) {
    return (values[mid - 1] + values[mid]) / 2;
  } else {
    return values[mid];
  }
}

function findCenter(coords) {
  if (coords.length === 0) {
    return null;
  }

  const latitudes = coords.map(coord => coord[0]);
  const longitudes = coords.map(coord => coord[1]);

  const medianLat = findMedian(latitudes);
  const medianLng = findMedian(longitudes);

  return fromLonLat([medianLng, medianLat]);
}

const map = new Map({
  target: document.getElementById('map'),
  view: new View({
    center: findCenter(coords),
    zoom: 17.5,
    minZoom: 17.2,
    maxZoom: 17.2,
  }),
  layers: [
    new TileLayer({
      source: new OSMSource({
        attributions: 'Maps &copy; <a href="https://thunderforest.com" target="_blank">Thunderbird</a>, Data &copy; <a href="https://www.openstreetmap.org/copyright" target="_blank">OpenStreetMap contributors</a>',
        url: 'https://{a-c}.tile.thunderforest.com/cycle/{z}/{x}/{y}.png?apikey=' + key
      }),
    }),
    new VectorLayer({
      source: new VectorSource({
        features: [(new Feature({
                type: 'route',
                geometry: new LineString((coords.map((c) => fromLonLat(c.reverse())))),
            }))],
      }),
      style: new Style({
        stroke: new Stroke({
          width: 4,
          color: '#FF8400',
        }),
      }),
    })
  ],
  controls: [],
  interactions: []
});

// dark mode for the map
map.on('postcompose',function(_){
  document.querySelector('canvas').style.filter="invert(100%) hue-rotate(180deg)";
});

// add a title overlay if a title is provided
// const title = urlParams.get('title')
// console.log(title)
// if (title != null) {
//   const element = document.getElementById('title')
//   element.textContent = title;
//   document.title = title;
// }
