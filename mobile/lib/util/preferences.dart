import 'package:shared_preferences/shared_preferences.dart';

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const userSeedBackupConfirmed = "userSeedBackupConfirmed";
  static const emailAddress = "emailAddress";

  setUserSeedBackupConfirmed(bool value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setBool(userSeedBackupConfirmed, value);
  }

  Future<bool> isUserSeedBackupConfirmed() async {
    // FIXME: disabling the user seed backup confirmed preference so that the backup button is always visible. Eventually, we should think about how we want to make the seed backup accessible to the user at all times.
    // SharedPreferences preferences = await SharedPreferences.getInstance();
    // return preferences.getBool(userSeedBackupConfirmed) ?? false;
    return false;
  }

  setEmailAddress(String value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(emailAddress, value);
  }

  Future<String> getEmailAddress() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(emailAddress) ?? "";
  }

  Future<bool> hasEmailAddress() async {
    var email = await getEmailAddress();
    return email.isNotEmpty;
  }
}
