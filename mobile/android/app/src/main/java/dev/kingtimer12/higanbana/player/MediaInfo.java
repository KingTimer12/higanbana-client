package dev.kingtimer12.higanbana.player;

import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Faixas e capítulos do arquivo aberto, a partir das listas em JSON do mpv. */
final class MediaInfo {
    private static final String TAG = "higanbana.mpv";

    private MediaInfo() {}

    /** {@code track-list}: {@code [{ id, type, lang, title, selected }]}. */
    static List<Object> tracks(String json) {
        List<Object> out = new ArrayList<>();
        try {
            JSONArray list = new JSONArray(orEmpty(json));
            for (int i = 0; i < list.length(); i++) {
                JSONObject t = list.getJSONObject(i);
                Map<String, Object> track = new HashMap<>();
                track.put("id", t.optLong("id"));
                track.put("type", t.optString("type"));
                track.put("lang", t.has("lang") ? t.optString("lang") : null);
                track.put("title", t.has("title") ? t.optString("title") : null);
                track.put("selected", t.optBoolean("selected"));
                out.add(track);
            }
        } catch (JSONException e) {
            Log.w(TAG, "track-list ilegível", e);
        }
        return out;
    }

    /** {@code chapter-list}: {@code [{ title, time }]}. */
    static List<Object> chapters(String json) {
        List<Object> out = new ArrayList<>();
        try {
            JSONArray list = new JSONArray(orEmpty(json));
            for (int i = 0; i < list.length(); i++) {
                JSONObject c = list.getJSONObject(i);
                Map<String, Object> chapter = new HashMap<>();
                chapter.put("title", c.optString("title"));
                chapter.put("time", c.optDouble("time", 0));
                out.add(chapter);
            }
        } catch (JSONException e) {
            Log.w(TAG, "chapter-list ilegível", e);
        }
        return out;
    }

    private static String orEmpty(String json) {
        return json == null || json.isEmpty() ? "[]" : json;
    }
}
