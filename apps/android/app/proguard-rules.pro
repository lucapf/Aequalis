# WorkManager's database (Room) is found by name at start: R8 must keep it, or the app stops at
# start (`Failed to create an instance of class androidx.work.impl.WorkDatabase`).
-keep class * extends androidx.room.RoomDatabase { *; }
-keep class androidx.work.impl.WorkDatabase { *; }
-keep class androidx.work.impl.WorkDatabase_Impl { *; }
